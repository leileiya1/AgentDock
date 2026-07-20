import { useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { AnimatePresence, motion } from "motion/react";
import { ArrowDown, Settings2 } from "lucide-react";
import type { AgentEvent, AgentEventKind } from "@/generated/bindings";
import { useRunLog } from "@/hooks/useRunLog";
import { cn } from "@/lib/utils";
import { EmptyState } from "./EmptyState";
import { ErrorState } from "./ErrorState";

interface Props {
  runId: string;
  preferredSummary?: string;
}

const KIND_LABEL: Record<AgentEventKind, string> = {
  system: "系统", assistant_text: "AI", tool_use: "工具", tool_result: "结果", result: "结论", raw: "原始",
};
type LogFilter = AgentEventKind | "all" | "highlights" | "tools";
const MAIN_FILTERS: Array<{ id: LogFilter; label: string }> = [
  { id: "highlights", label: "主要内容" },
  { id: "assistant_text", label: "AI 文字" },
  { id: "result", label: "最终总结" },
];
const TECHNICAL_FILTERS: Array<{ id: LogFilter; label: string }> = [
  { id: "system", label: "系统" },
  { id: "tools", label: "工具调用" },
  { id: "raw", label: "原始" },
  { id: "all", label: "全部" },
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function parseJson(value: string | null): unknown {
  if (!value?.trim()) return null;
  try {
    return JSON.parse(value);
  } catch {
    return null;
  }
}

function fileChangeSummary(changes: unknown[]): string {
  const names = changes.flatMap((change) => {
    if (!isRecord(change) || typeof change.path !== "string") return [];
    return [change.path.split(/[\\/]/).pop() || change.path];
  });
  return names.length > 0 ? `已修改：${names.slice(0, 4).join("、")}` : "已完成文件改动";
}

/**
 * Early Codex runs stored the provider JSON envelope as assistant text. Reclassify
 * those historical rows at display time so old tasks benefit from the new log UI too.
 */
function normalizeLegacyCodexEvent(ev: AgentEvent): AgentEvent {
  const summary = ev.summary.trim();
  if (!summary.startsWith("{") && !summary.startsWith("[")) return ev;

  const envelope = parseJson(ev.text) ?? parseJson(ev.summary);
  if (Array.isArray(envelope)) {
    return { ...ev, kind: "tool_use", summary: fileChangeSummary(envelope) };
  }
  if (!isRecord(envelope)) return ev;

  if (typeof envelope.summary === "string") {
    return { ...ev, kind: "result", summary: envelope.summary };
  }

  const eventType = typeof envelope.type === "string" ? envelope.type : "";
  if (eventType === "turn.completed") {
    return { ...ev, kind: "result", summary: "本轮处理完成" };
  }
  if (eventType === "thread.started" || eventType === "turn.started") {
    return { ...ev, kind: "system", summary: eventType === "thread.started" ? "Codex 会话已启动" : "开始处理任务" };
  }

  const item = isRecord(envelope.item) ? envelope.item : null;
  if (!item) return ev;
  const itemType = typeof item.type === "string" ? item.type : "";

  if (itemType === "agent_message") {
    const text = typeof item.text === "string" ? item.text : "Agent 已更新进展";
    const structured = parseJson(text);
    const finalSummary = isRecord(structured) && typeof structured.summary === "string" ? structured.summary : null;
    return { ...ev, kind: finalSummary ? "result" : "assistant_text", summary: finalSummary ?? text };
  }
  if (itemType === "command_execution") {
    const failed = item.status === "failed";
    return {
      ...ev,
      kind: eventType === "item.started" ? "tool_use" : "tool_result",
      summary: eventType === "item.started" ? "正在执行命令" : failed ? "命令执行失败" : "命令执行完成",
    };
  }
  if (itemType === "file_change") {
    return { ...ev, kind: "tool_use", summary: fileChangeSummary(Array.isArray(item.changes) ? item.changes : []) };
  }
  if (itemType === "web_search") {
    return { ...ev, kind: "tool_use", summary: "已完成资料搜索" };
  }
  return ev;
}

function LogRow({ ev, allowExpand }: { ev: AgentEvent; allowExpand: boolean }) {
  const [open, setOpen] = useState(false);
  const canExpand = allowExpand && !!ev.text && ev.text !== ev.summary;
  const stderr = ev.stream === "stderr";
  const technical = ["tool_use", "tool_result", "raw"].includes(ev.kind);
  return (
    <div className={cn("px-4 text-[13px] leading-relaxed", technical && "bg-panel/60 font-mono text-[12px]")}>
      <button
        type="button"
        onClick={() => canExpand && setOpen((o) => !o)}
        disabled={!canExpand}
        className="flex w-full items-baseline gap-2 py-1 text-left"
      >
        <span className="w-8 shrink-0 text-[10px] font-medium uppercase text-t3">{KIND_LABEL[ev.kind]}</span>
        <span
          className={cn(
            "min-w-0 flex-1 whitespace-pre-wrap break-words",
            ev.kind === "tool_use" && "truncate whitespace-nowrap text-t2",
            ev.kind === "result" && "font-medium text-t1",
            ev.kind === "system" && "text-t3",
            stderr && "text-bad",
            !stderr && ev.kind !== "tool_use" && ev.kind !== "result" && ev.kind !== "system" && "text-t1"
          )}
        >
          {ev.summary}
        </span>
        {canExpand && <span className="shrink-0 text-t3">{open ? "▾" : "▸"}</span>}
      </button>
      {open && ev.text && (
        <pre className="mb-2 ml-11 max-h-80 overflow-auto whitespace-pre-wrap break-words rounded-md border border-line bg-app px-2 py-2 text-t2">
          {ev.text}
        </pre>
      )}
    </div>
  );
}

/** Virtualized log list with bottom-stick + "回到最新" pill (02 §6/§7). */
export function RunLogViewer({ runId, preferredSummary }: Props) {
  const { buffer, loading, error, loadMore, hasMore } = useRunLog(runId);
  const [filter, setFilter] = useState<LogFilter>("highlights");
  const [technicalOpen, setTechnicalOpen] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const stuckRef = useRef(true);
  const [stuck, setStuck] = useState(true);

  const lines = buffer?.lines ?? [];
  const displayLines = useMemo(() => lines.map(normalizeLegacyCodexEvent), [lines]);
  const filtered = useMemo(() => {
    if (filter === "all") return displayLines;
    if (filter === "tools") return displayLines.filter((line) => line.kind === "tool_use" || line.kind === "tool_result");
    if (filter === "highlights") {
      if (preferredSummary?.trim()) {
        return [{
          ts: "",
          stream: "stdout" as const,
          kind: "result" as const,
          summary: preferredSummary.trim(),
          text: null,
        }];
      }
      // Completed runs lead with one useful conclusion. Some CLIs emit a generic
      // "completed" event after their real final message, so prefer meaningful
      // result text and then the latest AI narrative instead of showing both.
      const results = displayLines.filter((line) => line.kind === "result");
      const meaningfulResult = results.slice().reverse().find((line) =>
        !["本轮处理完成", "运行完成", "处理完成"].includes(line.summary)
        && !line.summary.endsWith("returned structured review")
      );
      if (meaningfulResult) return [meaningfulResult];
      const narrative = displayLines.filter((line) => line.kind === "assistant_text");
      if (results.length > 0) return narrative.length > 0 ? [narrative[narrative.length - 1]] : [results[results.length - 1]];
      return narrative.slice(-6);
    }
    return displayLines.filter((line) => line.kind === filter);
  }, [displayLines, filter, preferredSummary]);
  const technicalFilter = ["system", "tools", "raw", "all"].includes(filter);

  const virtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 26,
    overscan: 24,
    measureElement: (el) => el.getBoundingClientRect().height,
  });

  useEffect(() => {
    if (stuckRef.current && filtered.length > 0) virtualizer.scrollToIndex(filtered.length - 1, { align: "end" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filtered.length]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
    stuckRef.current = atBottom;
    if (atBottom !== stuck) setStuck(atBottom);
    if (el.scrollTop < 40 && hasMore && !loading) loadMore();
  };

  const jumpToLatest = () => {
    stuckRef.current = true;
    setStuck(true);
    if (filtered.length > 0) virtualizer.scrollToIndex(filtered.length - 1, { align: "end" });
  };

  if (error && lines.length === 0) return <ErrorState error={error} onRetry={loadMore} />;

  return (
    <div className="relative flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-line/70 px-3 py-2">
        <div className="flex flex-wrap gap-1" role="tablist" aria-label="日志过滤">
          {MAIN_FILTERS.map((f) => (
            <button
              key={f.id}
              role="tab"
              aria-selected={filter === f.id}
              onClick={() => setFilter(f.id)}
              className={cn(
                "rounded-full border px-2.5 py-0.5 text-[12px] transition-colors",
                filter === f.id ? "border-line-strong bg-raised text-t1" : "border-line text-t2 hover:text-t1"
              )}
            >
              {f.label}
            </button>
          ))}
          {technicalOpen && TECHNICAL_FILTERS.map((f) => (
            <button
              key={f.id}
              role="tab"
              aria-selected={filter === f.id}
              onClick={() => setFilter(f.id)}
              className={cn(
                "rounded-full border px-2.5 py-0.5 text-[12px] transition-colors",
                filter === f.id ? "border-line-strong bg-raised text-t1" : "border-line text-t3 hover:text-t1"
              )}
            >
              {f.label}
            </button>
          ))}
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => {
              setTechnicalOpen((open) => {
                if (open && technicalFilter) setFilter("highlights");
                return !open;
              });
            }}
            className={cn(
              "flex items-center gap-1 rounded-md px-2 py-1 text-[12px] transition-colors",
              technicalOpen ? "bg-raised text-t2" : "text-t3 hover:bg-raised hover:text-t1"
            )}
          >
            <Settings2 className="size-3.5" /> {technicalOpen ? "收起技术日志" : "技术日志"}
          </button>
          <span className="text-[12px] text-t3">{filtered.length} 条</span>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto py-2" ref={scrollRef} onScroll={onScroll}>
        {buffer?.headTrimmed && (
          <button
            onClick={loadMore}
            disabled={loading}
            className="block w-full border-b border-line/70 py-2 text-center text-[12px] text-t3 hover:text-t1"
          >
            {loading ? "加载中…" : "已省略更早输出，点击加载"}
          </button>
        )}
        {filtered.length === 0 && !loading ? (
          <div className="p-6">
            <EmptyState title="还没有主要内容" hint="运行中会显示 AI 的文字进展，完成后会优先显示最终总结。" />
          </div>
        ) : (
          <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
            {virtualizer.getVirtualItems().map((vi) => (
              <div
                key={vi.key}
                data-index={vi.index}
                ref={virtualizer.measureElement}
                style={{ position: "absolute", top: 0, left: 0, width: "100%", transform: `translateY(${vi.start}px)` }}
              >
                <LogRow ev={filtered[vi.index]} allowExpand={technicalOpen} />
              </div>
            ))}
          </div>
        )}
      </div>

      <AnimatePresence>
        {!stuck && (
          <motion.button
            onClick={jumpToLatest}
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 8 }}
            className="absolute bottom-3 right-3 flex items-center gap-1 rounded-full bg-run px-3 py-1.5 text-[12px] font-semibold text-white shadow-[var(--shadow-float)]"
          >
            回到最新 <ArrowDown className="size-3.5" />
          </motion.button>
        )}
      </AnimatePresence>
    </div>
  );
}
