import type { EnvReport, OnboardingReport, ProviderStatus, ToolStatus } from "@/generated/bindings";

export type DotTone = "ok" | "bad" | "idle";
export interface Dot {
  key: string;
  label: string;
  tone: DotTone;
  blocking: boolean;
}

/**
 * A CLI is "就绪" only when it is installed, protocol-compatible AND authenticated. A found but
 * not-logged-in CLI (authenticated === false) must read as a blocker, so the sidebar can never show
 * "环境就绪" while the developer/reviewer CLI cannot actually run (问题清单 P0-01).
 */
export function toolDot(key: string, label: string, s: ToolStatus, optional: boolean): Dot {
  if (s.found && s.compatible && s.authenticated !== false) {
    if (s.supportLevel === "compatible_untested") {
      return { key, label: `${label}：参数兼容，任务开始前会真实探测`, tone: "idle", blocking: false };
    }
    return { key, label: `${label}：就绪`, tone: "ok", blocking: false };
  }
  if (!s.found && optional) {
    return { key, label: `${label}：适配器已就绪，可稍后安装`, tone: "idle", blocking: false };
  }
  const reason =
    s.authenticated === false
      ? s.authProblem ?? `${label} 已安装但尚未登录`
      : s.problem ?? "未就绪";
  return { key, label: `${label}：${reason}`, tone: "bad", blocking: !optional };
}

export function providerDot(key: string, label: string, s: ProviderStatus): Dot {
  if (s.available) return { key, label: `${label}：可用`, tone: "ok", blocking: false };
  if (s.configured) return { key, label: `${label}：${s.problem ?? "凭据不可用"}`, tone: "bad", blocking: false };
  return { key, label: `${label}：未配置`, tone: "idle", blocking: false };
}

export function buildDots(env: EnvReport | undefined, daemonRunning: boolean | undefined): Dot[] {
  if (!env) return [];
  return [
    { key: "daemon", label: daemonRunning ? "调度服务：运行中" : "调度服务：未运行", tone: daemonRunning ? "ok" : "bad", blocking: !daemonRunning },
    toolDot("git", "Git", env.git, false),
    toolDot("claude", "Claude Code", env.claudeCode, false),
    toolDot("codex", "Codex", env.codex, false),
    toolDot("gemini", "Gemini CLI", env.geminiCli, true),
    toolDot("qwen", "Qwen Code", env.qwenCode, true),
    toolDot("qoder", "Qoder CLI", env.qoderCli, true),
    toolDot("grok", "Grok CLI", env.grokCli, true),
    providerDot("openai", "OpenAI API", env.openaiApi),
    providerDot("anthropic", "Anthropic API", env.anthropicApi),
    providerDot("deepseek", "DeepSeek API", env.deepseekApi),
  ];
}

export interface EnvironmentSummary {
  label: string;
  detail: string;
  tone: DotTone;
  blocking: boolean;
}

/** One truthful environment summary shared by the sidebar and project toolbar. */
export function summarizeEnvironment(report: OnboardingReport | undefined): EnvironmentSummary {
  if (!report) {
    return { label: "正在检查环境", detail: "正在读取本机工具与调度服务状态", tone: "idle", blocking: false };
  }

  const dots = buildDots(report.env, report.daemonRunning);
  const firstBlocker = dots.find((dot) => dot.blocking);
  const blocking = !report.workflowReady || !report.daemonRunning || firstBlocker != null;

  if (!blocking) {
    return { label: "端到端环境就绪", detail: "开发、审查与调度链路可运行", tone: "ok", blocking: false };
  }

  const detail = firstBlocker?.label ?? report.notices[0] ?? "进入设置查看阻塞项";
  if (report.appReady && !report.workflowReady) {
    return { label: "应用可用 · 工作流未就绪", detail, tone: "bad", blocking: true };
  }
  return { label: "环境有阻塞项", detail, tone: "bad", blocking: true };
}
