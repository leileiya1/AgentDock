use super::*;

#[test]
fn extracts_openai_compatible_usage_and_estimates_configured_cost() {
    let mut settings = ApiProviderSettings::deepseek_default();
    settings.input_cost_per_million = Some(1.0);
    settings.output_cost_per_million = Some(2.0);
    let adapter = ApiProviderAdapter::new(AgentKind::DeepSeekApi, settings);
    let (input, output, cost) = adapter.extract_telemetry(&json!({
        "usage": {"prompt_tokens": 1000, "completion_tokens": 500}
    }));
    assert_eq!((input, output), (Some(1000), Some(500)));
    assert_eq!(cost, Some(0.002));
}

#[test]
fn provider_reported_cost_wins_over_estimate_and_anthropic_cache_is_counted() {
    let adapter = ApiProviderAdapter::new(
        AgentKind::AnthropicApi,
        ApiProviderSettings::anthropic_default(),
    );
    let (input, output, cost) = adapter.extract_telemetry(&json!({
        "usage": {
            "input_tokens": 100,
            "cache_creation_input_tokens": 30,
            "cache_read_input_tokens": 20,
            "output_tokens": 10,
            "cost_usd": 0.03
        }
    }));
    assert_eq!((input, output), (Some(150), Some(10)));
    assert_eq!(cost, Some(0.03));
}

#[test]
fn api_budget_caps_output_and_fails_before_network_when_context_does_not_fit() {
    let mut settings = ApiProviderSettings::deepseek_default();
    settings.max_output_tokens = 8_000;
    settings.input_cost_per_million = Some(0.0);
    settings.output_cost_per_million = Some(1_000_000.0);
    let adapter = ApiProviderAdapter::new(AgentKind::DeepSeekApi, settings);
    let cap = match adapter.budgeted_max_output_tokens(
        "small prompt",
        &RunBudget {
            remaining_tokens: Some(1_000_000),
            remaining_cost_usd: Some(3.9),
        },
    ) {
        Ok(cap) => cap,
        Err(error) => panic!("priced request should fit: {error}"),
    };
    assert_eq!(cap, 3);
    let error = match adapter.budgeted_max_output_tokens(
        "context",
        &RunBudget {
            remaining_tokens: Some(1),
            remaining_cost_usd: None,
        },
    ) {
        Err(error) => error,
        Ok(_) => panic!("oversized context was allowed to reach the network"),
    };
    assert!(error.to_string().contains("BUDGET_EXCEEDED"));
    assert_eq!(adapter.budget_capabilities().tokens, BudgetMode::Hard);
    assert_eq!(adapter.budget_capabilities().cost, BudgetMode::Hard);
}
