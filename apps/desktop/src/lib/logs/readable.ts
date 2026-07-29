import type { AgentEvent, AgentEventKind } from "@/generated/bindings";

/**
 * 日志内容归一化 (05 §4.2, §10). Every Provider — Codex, Claude, Gemini, Qwen, the API
 * providers and historical rows — goes through this one policy, so "主要内容" can never
 * show JSON、shell 命令、协议字段 or 临时路径.
 *
 * The hard rule: when extraction fails we return `null`, never the raw payload.
 * Falling back to raw JSON is exactly what P0-02 was about.
 */

export interface ReadableEvent {
  /** 归一化后的用途，与后端 AgentEventKind 解耦。 */
  kind: "narrative" | "result" | "tool" | "system";
  /** 可读文字；`null` 表示没能提取出可读内容（主视图显示提示，不显示原文）。 */
  text: string | null;
  /** 是否允许出现在「主要内容」。 */
  mainView: boolean;
  original: AgentEvent;
}

const MAX_JSON_DEPTH = 6;

export function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function tryParse(value: string): unknown {
  const trimmed = value.trim();
  if (!trimmed.startsWith("{") && !trimmed.startsWith("[")) return undefined;
  try {
    return JSON.parse(trimmed);
  } catch {
    return undefined;
  }
}

/**
 * Providers routinely double-encode: a JSON envelope whose `text` is itself a JSON
 * string whose `summary` is a third. 递归解析嵌套 JSON 字符串 (05 §4.2) until it stops
 * being JSON, so extraction sees real values instead of escaped blobs.
 */
export function unwrapJson(value: unknown, depth = 0): unknown {
  if (depth >= MAX_JSON_DEPTH) return value;
  if (typeof value === "string") {
    const parsed = tryParse(value);
    return parsed === undefined ? value : unwrapJson(parsed, depth + 1);
  }
  return value;
}

/** 候选字段按「最像最终结论」到「最像片段」排列 (05 §4.2)。 */
const TEXT_CANDIDATES = [
  ["agent_message", "text"],
  ["aggregated_output", "summary"],
  ["result", "summary"],
  // Codex wraps the message as {item: {type: "agent_message", text}}.
  ["item", "text"],
  ["item", "summary"],
  ["summary"],
  ["final_message"],
  ["message"],
  ["text"],
  ["content"],
  ["output"],
] as const;

function readPath(source: unknown, path: readonly string[]): unknown {
  let cursor: unknown = source;
  for (const key of path) {
    if (!isRecord(cursor)) return undefined;
    cursor = cursor[key];
  }
  return cursor;
}

/**
 * Pull the best human sentence out of an arbitrarily shaped payload.
 * Returns `null` rather than something JSON-shaped.
 */
export function extractReadableText(value: unknown, depth = 0): string | null {
  const unwrapped = unwrapJson(value, depth);

  if (typeof unwrapped === "string") {
    const text = unwrapped.trim();
    if (!text || looksLikeProtocol(text)) return null;
    return text;
  }

  if (Array.isArray(unwrapped)) {
    // Anthropic-style content blocks: [{type:"text", text:"…"}]
    const parts = unwrapped
      .map((item) => (isRecord(item) && typeof item.text === "string" ? item.text.trim() : null))
      .filter((part): part is string => !!part);
    return parts.length > 0 ? parts.join("\n") : null;
  }

  if (!isRecord(unwrapped)) return null;

  for (const path of TEXT_CANDIDATES) {
    const candidate = unwrapJson(readPath(unwrapped, path), depth + 1);
    if (typeof candidate === "string" && candidate.trim() && !looksLikeProtocol(candidate)) {
      return candidate.trim();
    }
    if (isRecord(candidate) || Array.isArray(candidate)) {
      const nested = extractReadableText(candidate, depth + 1);
      if (nested) return nested;
    }
  }
  return null;
}

/** 协议字段与 JSON 原文的特征 (05 §4.2)。 */
const PROTOCOL_MARKERS = [
  "item.completed",
  "item.started",
  "command_execution",
  "thread.started",
  "turn.completed",
  "turn.started",
  "tool_use",
  "tool_result",
  "function_call",
  "\"type\":",
  "\"schema\"",
];

export function looksLikeProtocol(text: string): boolean {
  const trimmed = text.trim();
  if (!trimmed) return true;
  // 原始 JSON / JSONL，无论是否转义过。
  if (/^[[{]/.test(trimmed) && (trimmed.includes('":') || trimmed.includes("\\\""))) return true;
  const lower = trimmed.toLowerCase();
  return PROTOCOL_MARKERS.some((marker) => lower.includes(marker.toLowerCase()));
}

const REDACTIONS: Array<[RegExp, string]> = [
  // 临时目录与内部 run 目录的绝对路径。
  [/(?:\/private)?\/(?:var\/folders|tmp)\/[^\s"'`)]+/g, "（临时文件）"],
  [/[A-Za-z]:\\[^\s"'`)]*\\Temp\\[^\s"'`)]+/g, "（临时文件）"],
  [/[^\s"'`)]*[/\\]\.agentflow[/\\][^\s"'`)]+/g, "（运行目录）"],
  // task UUID / run id。
  [/\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b/gi, "（内部编号）"],
  // sha256 摘要。
  [/\b[0-9a-f]{64}\b/gi, "（校验值）"],
];

/**
 * 保留 Agent 的原话，但把临时路径、内部编号这类噪声换成可读占位 (05 §4.2)。
 * 整段删掉会连带丢掉用户真正需要的说明，所以这里做替换而不是丢弃。
 */
export function redactNoise(text: string): string {
  return REDACTIONS.reduce((acc, [pattern, replacement]) => acc.replace(pattern, replacement), text);
}

/** 这些通用结束语没有信息量，不能当作「本次结果」 (05 §4.4)。 */
const FILLER_RESULTS = new Set([
  "本轮处理完成",
  "运行完成",
  "处理完成",
  "完成",
  "done",
  "ok",
  "success",
]);

export function isFiller(text: string): boolean {
  const normalized = text.trim().toLowerCase();
  return FILLER_RESULTS.has(normalized) || normalized.endsWith("returned structured review");
}

const KIND_MAP: Record<AgentEventKind, ReadableEvent["kind"]> = {
  system: "system",
  assistant_text: "narrative",
  tool_use: "tool",
  tool_result: "tool",
  result: "result",
  raw: "system",
};

/**
 * Normalize one raw log line. Handles the historical Codex rows that stored the whole
 * provider envelope in `summary`, so old tasks benefit from the same policy (05 §4.4).
 */
export function toReadable(event: AgentEvent): ReadableEvent {
  const envelopeText = event.summary.trim();
  const envelope = /^[[{]/.test(envelopeText)
    ? unwrapJson(event.text ?? event.summary)
    : undefined;

  if (envelope !== undefined && (isRecord(envelope) || Array.isArray(envelope))) {
    return fromEnvelope(event, envelope);
  }

  const kind = KIND_MAP[event.kind];
  const readable = looksLikeProtocol(envelopeText) ? null : redactNoise(envelopeText);
  return {
    kind,
    text: readable,
    // 工具调用与系统行属于技术详情，永远不进主视图 (05 §4.2)。
    mainView: (kind === "narrative" || kind === "result") && !!readable,
    original: event,
  };
}

/** Reclassify a provider envelope that was stored as text. */
function fromEnvelope(event: AgentEvent, envelope: unknown): ReadableEvent {
  if (Array.isArray(envelope)) {
    return { kind: "tool", text: fileChangeSummary(envelope), mainView: false, original: event };
  }
  const record = envelope as Record<string, unknown>;
  const item = isRecord(record.item) ? record.item : null;
  const itemType = typeof item?.type === "string" ? item.type : "";
  const eventType = typeof record.type === "string" ? record.type : "";

  if (itemType === "command_execution") {
    const failed = item?.status === "failed";
    return {
      kind: "tool",
      text: failed ? "命令执行失败" : eventType === "item.started" ? "正在执行命令" : "命令执行完成",
      mainView: false,
      original: event,
    };
  }
  if (itemType === "file_change") {
    const changes = Array.isArray(item?.changes) ? item!.changes : [];
    return { kind: "tool", text: fileChangeSummary(changes), mainView: false, original: event };
  }
  if (itemType === "web_search") {
    return { kind: "tool", text: "已完成资料搜索", mainView: false, original: event };
  }
  if (eventType === "thread.started" || eventType === "turn.started") {
    return { kind: "system", text: "会话已开始", mainView: false, original: event };
  }

  if (itemType === "agent_message") {
    // The agent's message is sometimes itself the structured result JSON; if it carries
    // a `summary` it is the run's conclusion rather than running commentary.
    const inner = unwrapJson(item?.text);
    const structured = isRecord(inner) && typeof inner.summary === "string";
    const text = extractReadableText(item?.text ?? item);
    return {
      kind: structured ? "result" : "narrative",
      text: text ? redactNoise(text) : null,
      mainView: !!text,
      original: event,
    };
  }

  const text = extractReadableText(record);
  const isResult = eventType === "turn.completed" || typeof record.summary === "string";
  return {
    kind: isResult ? "result" : "narrative",
    text: text ? redactNoise(text) : eventType === "turn.completed" ? "本轮处理完成" : null,
    mainView: !!text,
    original: event,
  };
}

function fileChangeSummary(changes: unknown[]): string {
  const names = changes.flatMap((change) => {
    if (!isRecord(change) || typeof change.path !== "string") return [];
    return [change.path.split(/[\\/]/).pop() || change.path];
  });
  return names.length > 0 ? `已修改：${names.slice(0, 4).join("、")}` : "已完成文件改动";
}

export function toReadableAll(events: AgentEvent[]): ReadableEvent[] {
  return events.map(toReadable);
}
