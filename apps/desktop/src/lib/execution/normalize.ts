import type { Actor, TaskEvent } from "@/generated/bindings";
import { eventCopy, isSystemDetail, type EventCopy } from "@/copy/events";

/**
 * 数据归一化层 (05 §10). Turns raw `TaskEvent` rows into a shape the tree model can
 * consume without ever touching `event_type` again. Pure — no React, no formatting.
 */
export interface NormalizedEvent {
  id: number;
  runId: string | null;
  revision: number | null;
  actor: Actor;
  ts: string;
  /** Retained for 技术详情 / 导出 only; never rendered as user copy. */
  eventType: string;
  payload: Record<string, unknown> | null;
  copy: EventCopy;
  /** Default-hidden internal event (scheduler slot、result 文件、心跳…). */
  systemDetail: boolean;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

/**
 * Some payloads carry the revision even when the row's own column is null (e.g.
 * `scheduler:slot` writes `{"revision": n}`), so prefer the column and fall back
 * to the payload rather than dropping the event out of its revision.
 */
function revisionOf(event: TaskEvent, payload: Record<string, unknown> | null): number | null {
  if (event.revision != null) return event.revision;
  const fromPayload = payload?.revision;
  return typeof fromPayload === "number" ? fromPayload : null;
}

/**
 * The backend does not stamp `run_id` on every run-scoped event (provider fallback and
 * council failures only carry role/agent), so the tree model resolves those by
 * (revision, role/agent) instead. Expose the hints here so it stays a pure lookup.
 */
export function runHint(event: NormalizedEvent): { role: string | null; agent: string | null } {
  const role = event.payload?.role;
  // `from` is the Provider that failed — the fallback belongs under *its* run.
  const agent = event.payload?.from ?? event.payload?.agent;
  return {
    role: typeof role === "string" ? role : null,
    agent: typeof agent === "string" ? agent : null,
  };
}

export function normalizeEvents(events: TaskEvent[]): NormalizedEvent[] {
  return events.map((event) => {
    const payload = asRecord(event.payload);
    const copy = eventCopy(event.eventType, payload);
    return {
      id: event.id,
      runId: event.runId,
      revision: revisionOf(event, payload),
      actor: event.actor,
      ts: event.createdAt,
      eventType: event.eventType,
      payload,
      copy,
      systemDetail: isSystemDetail(copy),
    };
  });
}
