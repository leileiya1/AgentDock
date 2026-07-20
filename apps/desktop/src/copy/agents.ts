import type { AgentKind } from "@/generated/bindings";

interface AgentMeta {
  /** short human label */
  label: string;
  /** single-char mark for AgentMark (no official logos) */
  mark: string;
  /** css color token name for --agent-* families, or a fallback */
  colorVar: string;
  cli: boolean;
}

export const AGENT_META: Record<string, AgentMeta> = {
  claude_code: { label: "Claude Code", mark: "C", colorVar: "--agent-claude", cli: true },
  codex: { label: "Codex", mark: "X", colorVar: "--agent-codex", cli: true },
  gemini_cli: { label: "Gemini CLI", mark: "G", colorVar: "--agent-gemini", cli: true },
  qwen_code: { label: "Qwen Code", mark: "Q", colorVar: "--agent-qwen", cli: true },
  openai_api: { label: "OpenAI API", mark: "O", colorVar: "--agent-gemini", cli: false },
  anthropic_api: { label: "Anthropic API", mark: "A", colorVar: "--agent-claude", cli: false },
  deepseek_api: { label: "DeepSeek API", mark: "D", colorVar: "--agent-qwen", cli: false },
  grok_api: { label: "Grok API", mark: "X", colorVar: "--agent-codex", cli: false },
  minimax_api: { label: "MiniMax API", mark: "M", colorVar: "--agent-qwen", cli: false },
  kimi_api: { label: "Kimi API", mark: "K", colorVar: "--agent-gemini", cli: false },
};

export const ALL_AGENTS: AgentKind[] = [
  "claude_code",
  "codex",
  "gemini_cli",
  "qwen_code",
  "openai_api",
  "anthropic_api",
  "deepseek_api",
  "grok_api",
  "minimax_api",
  "kimi_api",
];

export function agentLabel(kind: AgentKind | null | undefined): string {
  if (!kind) return "—";
  return AGENT_META[kind]?.label ?? kind;
}

const API_AGENTS = new Set([
  "openai_api",
  "anthropic_api",
  "deepseek_api",
  "grok_api",
  "minimax_api",
  "kimi_api",
]);

export function isApiAgent(kind: AgentKind): boolean {
  return API_AGENTS.has(kind);
}

/** External Provider IDs have no compile-time artwork, so use a stable neutral letter mark. */
export function agentMeta(kind: AgentKind): AgentMeta {
  return AGENT_META[kind] ?? {
    label: kind,
    mark: kind.slice(0, 1).toUpperCase() || "P",
    colorVar: "--color-t2",
    cli: false,
  };
}
