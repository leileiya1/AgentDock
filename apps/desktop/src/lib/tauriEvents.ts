import { useEffect, useRef } from "react";
import { listen, type Event as TauriEvent } from "@tauri-apps/api/event";
import type {
  AgentEvent,
  EnvReport,
  ErrorCode,
  RunSummary,
  TaskStatus,
  TaskSummary,
} from "@/generated/bindings";

/*
 * Typed Tauri event layer (03 §4). tauri-specta currently emits only `commands`
 * into generated/bindings.ts, so the Rust→UI event channel is subscribed here
 * with the payload shapes fixed by the contract. If a generated `events` object
 * is added later this module becomes a thin re-export.
 */

export interface TaskChangedPayload {
  task: TaskSummary;
  from: TaskStatus;
  to: TaskStatus;
}

export interface RunLogPayload {
  runId: string;
  batch: AgentEvent[];
}

export interface TaskRemovedPayload {
  taskId: string;
  projectId: string;
}

export interface AppErrorPayload {
  scope: string;
  code: ErrorCode;
  message: string;
}

export interface TauriEventMap {
  "task:changed": TaskChangedPayload;
  "task:removed": TaskRemovedPayload;
  "run:started": RunSummary;
  "run:log": RunLogPayload;
  "run:finished": RunSummary;
  "env:changed": EnvReport;
  "app:error": AppErrorPayload;
}

export type TauriEventName = keyof TauriEventMap;

/**
 * Subscribe to a Tauri event with automatic cleanup and StrictMode-safe
 * double-mount handling. The handler is read from a ref so subscription is not
 * torn down when the handler identity changes between renders.
 */
export function useTauriEvent<K extends TauriEventName>(
  name: K,
  handler: (payload: TauriEventMap[K]) => void,
  enabled = true
): void {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    if (!enabled) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;

    listen<TauriEventMap[K]>(name, (event: TauriEvent<TauriEventMap[K]>) => {
      handlerRef.current(event.payload);
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch((err) => {
        // In a plain browser (no tauri host) listen rejects — dev-only noise.
        console.warn(`[tauriEvents] failed to subscribe to ${name}:`, err);
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [name, enabled]);
}
