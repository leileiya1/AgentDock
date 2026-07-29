import type { AgentEvent } from "@/generated/bindings";
import {
  extractReadableText,
  isFiller,
  isRecord,
  looksLikeProtocol,
  redactNoise,
  toReadableAll,
  unwrapJson,
  type ReadableEvent,
} from "./readable";

/**
 * 结果优先的内容模型 (05 §4.1). 「本次结果」是一句话结论，「主要内容」是
 * 做了什么 / 验证 / 仍需注意 / 下一步——全部来自结构化结果，
 * 而不是碰运气取最后一行日志 (05 §4.4)。
 */
export interface ResultCard {
  /** 一句话结论。 */
  conclusion: string;
  completed: string[];
  validation: string[];
  concerns: string[];
  nextAction: string | null;
  /** 结论是否来自结构化结果；false 表示退回到 Agent 的自然语言总结。 */
  structured: boolean;
}

const LIST_KEYS = {
  completed: ["changes", "changed_files", "completed", "done", "work"],
  validation: ["tests", "validation", "checks", "verification"],
  concerns: ["issues", "concerns", "risks", "notes", "limitations"],
} as const;

const NEXT_KEYS = ["next_action", "nextAction", "next", "follow_up", "followUp"];

/** 把候选字段规整成短句列表，丢掉 JSON 片段。 */
function toLines(value: unknown): string[] {
  const unwrapped = unwrapJson(value);
  if (typeof unwrapped === "string") {
    return unwrapped
      .split(/\r?\n|[;；]/)
      .map((line) => line.replace(/^[-•*\d.)\s]+/, "").trim())
      .filter((line) => line.length > 0 && !looksLikeProtocol(line))
      .map(redactNoise);
  }
  if (Array.isArray(unwrapped)) {
    return unwrapped
      .flatMap((item) => {
        if (typeof item === "string") return [item.trim()];
        if (isRecord(item)) {
          const text = extractReadableText(item);
          if (text) return [text];
          // 文件改动列表：{path, insertions, deletions}
          if (typeof item.path === "string") return [item.path];
        }
        return [];
      })
      .filter((line) => line.length > 0 && !looksLikeProtocol(line))
      .map(redactNoise);
  }
  return [];
}

function pickList(source: Record<string, unknown>, keys: readonly string[]): string[] {
  for (const key of keys) {
    if (!(key in source)) continue;
    const lines = toLines(source[key]);
    if (lines.length > 0) return lines;
  }
  return [];
}

/**
 * Providers bury the structured result at different depths — bare at the top level,
 * under `result`, or as a JSON string inside a Codex `item.text`. Search recursively for
 * the first object carrying a non-empty `summary` rather than hard-coding each shape.
 */
function findStructured(value: unknown, depth = 0): Record<string, unknown> | null {
  if (depth > 6) return null;
  const unwrapped = unwrapJson(value, depth);

  if (Array.isArray(unwrapped)) {
    for (const item of unwrapped) {
      const found = findStructured(item, depth + 1);
      if (found) return found;
    }
    return null;
  }
  if (!isRecord(unwrapped)) return null;
  if (typeof unwrapped.summary === "string" && unwrapped.summary.trim() && !looksLikeProtocol(unwrapped.summary)) {
    return unwrapped;
  }
  for (const nested of Object.values(unwrapped)) {
    const found = findStructured(nested, depth + 1);
    if (found) return found;
  }
  return null;
}

/** Find the structured result object inside a run's log lines, newest first. */
function structuredResult(events: AgentEvent[]): Record<string, unknown> | null {
  for (let i = events.length - 1; i >= 0; i -= 1) {
    for (const raw of [events[i].text, events[i].summary]) {
      if (!raw) continue;
      const found = findStructured(raw);
      if (found) return found;
    }
  }
  return null;
}

/** 完成的运行优先展示结构化结果；没有结构化结果时退回到最后一段有意义的 AI 文字。 */
export function buildResultCard(events: AgentEvent[]): ResultCard | null {
  const structured = structuredResult(events);
  if (structured) {
    const conclusion = redactNoise(String(structured.summary).trim());
    return {
      conclusion,
      completed: pickList(structured, LIST_KEYS.completed),
      validation: pickList(structured, LIST_KEYS.validation),
      concerns: pickList(structured, LIST_KEYS.concerns),
      nextAction: pickNext(structured),
      structured: true,
    };
  }

  const readable = toReadableAll(events);
  const conclusion = fallbackConclusion(readable);
  if (!conclusion) return null;
  return { conclusion, completed: [], validation: [], concerns: [], nextAction: null, structured: false };
}

function pickNext(source: Record<string, unknown>): string | null {
  for (const key of NEXT_KEYS) {
    const lines = toLines(source[key]);
    if (lines.length > 0) return lines.join("；");
  }
  return null;
}

/**
 * 通用结束语（"本轮处理完成"）不算结论，要继续往前找真正的总结；
 * 都找不到时返回 null，由 UI 显示「未能提取可读总结」而不是塞回原始 JSON (05 §4.2)。
 */
function fallbackConclusion(readable: ReadableEvent[]): string | null {
  const meaningful = (event: ReadableEvent) => !!event.text && !isFiller(event.text);

  const result = readable.filter((e) => e.kind === "result" && meaningful(e)).at(-1);
  if (result?.text) return result.text;

  const narrative = readable.filter((e) => e.kind === "narrative" && meaningful(e)).at(-1);
  return narrative?.text ?? null;
}

/** 运行中的「实时进展」：人类可读短句，最近若干条 (05 §4.1)。 */
export function liveProgress(events: AgentEvent[], limit = 6): ReadableEvent[] {
  return toReadableAll(events)
    .filter((event) => event.mainView && event.text && !isFiller(event.text))
    .slice(-limit);
}
