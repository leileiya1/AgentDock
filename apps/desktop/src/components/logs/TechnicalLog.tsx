import { useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { AnimatePresence, motion } from "motion/react";
import { ArrowDown, Copy, Download, Search } from "lucide-react";
import type { AgentEvent, AgentEventKind } from "@/generated/bindings";
import { redactNoise } from "@/lib/logs/readable";
import { absoluteTime, copyText } from "@/lib/format";
import { toast } from "@/stores/toastStore";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { EmptyState } from "@/components/EmptyState";

const KIND_LABEL: Record<AgentEventKind, string> = {
  system: "系统",
  assistant_text: "AI",
  tool_use: "工具",
  tool_result: "结果",
  result: "结论",
  raw: "原始",
};

type KindFilter = AgentEventKind | "all";

const FILTERS: Array<{ id: KindFilter; label: string }> = [
  { id: "all", label: "全部" },
  { id: "assistant_text", label: "AI 文字" },
  { id: "tool_use", label: "工具调用" },
  { id: "tool_result", label: "工具结果" },
  { id: "system", label: "系统事件" },
  { id: "raw", label: "原始协议" },
];

/**
 * 技术详情 (05 §4.3). 技术能力一条都不删——系统事件、工具调用、命令、原始协议、
 * stdout/stderr 全在，只是默认后置。搜索、复制、导出**脱敏**日志、时间戳开关、
 * 自动跟随都在这里，10k 行仍使用虚拟列表 (05 §4.4)。
 */
export function TechnicalLog({
  runId,
  lines,
  headTrimmed,
  loading,
  hasMore,
  onLoadMore,
}: {
  runId: string;
  lines: AgentEvent[];
  headTrimmed: boolean;
  loading: boolean;
  hasMore: boolean;
  onLoadMore: () => void;
}) {
  const [filter, setFilter] = useState<KindFilter>("all");
  const [query, setQuery] = useState("");
  const [showTimestamps, setShowTimestamps] = useState(false);
  const [follow, setFollow] = useState(true);
  const followRef = useRef(true);
  const scrollRef = useRef<HTMLDivElement>(null);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return lines.filter((line) => {
      if (filter !== "all" && line.kind !== filter) return false;
      if (!needle) return true;
      return (
        line.summary.toLowerCase().includes(needle) ||
        (line.text?.toLowerCase().includes(needle) ?? false)
      );
    });
  }, [lines, filter, query]);

  const virtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 26,
    overscan: 24,
    measureElement: (el) => el.getBoundingClientRect().height,
  });

  useEffect(() => {
    if (followRef.current && filtered.length > 0) {
      virtualizer.scrollToIndex(filtered.length - 1, { align: "end" });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filtered.length]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
    followRef.current = atBottom;
    if (atBottom !== follow) setFollow(atBottom);
    if (el.scrollTop < 40 && hasMore && !loading) onLoadMore();
  };

  /** 导出的是脱敏日志 (05 §4.3)：临时路径、内部编号、校验值都已替换。 */
  const exportRedacted = () => {
    const body = filtered
      .map((line) => {
        const stamp = line.ts ? `[${absoluteTime(line.ts)}] ` : "";
        const detail = line.text && line.text !== line.summary ? `\n    ${redactNoise(line.text)}` : "";
        return `${stamp}${KIND_LABEL[line.kind]} · ${redactNoise(line.summary)}${detail}`;
      })
      .join("\n");
    const url = URL.createObjectURL(new Blob([body], { type: "text/plain;charset=utf-8" }));
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `run-${runId.slice(0, 8)}-脱敏日志.txt`;
    anchor.click();
    URL.revokeObjectURL(url);
    toast.info("已导出脱敏技术日志");
  };

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-line/70 px-3 py-2">
        <div className="relative">
          <Search className="pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-t3" aria-hidden />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="搜索日志"
            aria-label="搜索技术日志"
            className="h-7 w-44 pl-7 text-meta"
          />
        </div>
        <div className="flex flex-wrap gap-1" role="tablist" aria-label="日志类型">
          {FILTERS.map((item) => (
            <button
              key={item.id}
              role="tab"
              aria-selected={filter === item.id}
              onClick={() => setFilter(item.id)}
              className={cn(
                "rounded-pill border px-2 py-0.5 text-meta transition-colors",
                filter === item.id ? "border-line-strong bg-raised text-t1" : "border-line text-t3 hover:text-t1"
              )}
            >
              {item.label}
            </button>
          ))}
        </div>
        <label className="flex items-center gap-1 text-meta text-t3">
          <input
            type="checkbox"
            checked={showTimestamps}
            onChange={(e) => setShowTimestamps(e.target.checked)}
            className="accent-[var(--color-action)]"
          />
          时间戳
        </label>
        <label className="flex items-center gap-1 text-meta text-t3">
          <input
            type="checkbox"
            checked={follow}
            onChange={(e) => {
              followRef.current = e.target.checked;
              setFollow(e.target.checked);
            }}
            className="accent-[var(--color-action)]"
          />
          跟随最新
        </label>
        <span className="ml-auto flex items-center gap-1">
          <span className="text-meta tabular-nums text-t3">{filtered.length} 条</span>
          <Button
            variant="ghost"
            size="sm"
            onClick={async () => {
              const text = filtered.map((line) => redactNoise(line.summary)).join("\n");
              toast.info((await copyText(text)) ? "已复制脱敏日志" : "复制失败");
            }}
          >
            <Copy className="size-3.5" /> 复制
          </Button>
          <Button variant="ghost" size="sm" onClick={exportRedacted}>
            <Download className="size-3.5" /> 导出
          </Button>
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto py-1" ref={scrollRef} onScroll={onScroll}>
        {headTrimmed && (
          <button
            onClick={onLoadMore}
            disabled={loading}
            className="block w-full border-b border-line/70 py-2 text-center text-meta text-t3 hover:text-t1"
          >
            {loading ? "加载中…" : "已省略更早输出，点击加载"}
          </button>
        )}
        {filtered.length === 0 ? (
          <div className="p-6">
            <EmptyState
              title={query ? "没有匹配的日志" : "还没有技术日志"}
              hint={query ? "换一个关键词，或切换类型筛选。" : "运行开始后这里会记录系统事件、工具调用和原始协议。"}
            />
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
                <LogRow event={filtered[vi.index]} showTimestamp={showTimestamps} />
              </div>
            ))}
          </div>
        )}
      </div>

      <AnimatePresence>
        {!follow && (
          <motion.button
            onClick={() => {
              followRef.current = true;
              setFollow(true);
              if (filtered.length > 0) virtualizer.scrollToIndex(filtered.length - 1, { align: "end" });
            }}
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 8 }}
            className="absolute bottom-3 right-3 flex items-center gap-1 rounded-pill bg-action px-3 py-1.5 text-meta font-semibold text-white shadow-[var(--shadow-float)]"
          >
            回到最新 <ArrowDown className="size-3.5" />
          </motion.button>
        )}
      </AnimatePresence>
    </div>
  );
}

function LogRow({ event, showTimestamp }: { event: AgentEvent; showTimestamp: boolean }) {
  const [open, setOpen] = useState(false);
  const canExpand = !!event.text && event.text !== event.summary;
  const stderr = event.stream === "stderr";

  return (
    <div className="px-3 font-mono text-meta leading-relaxed">
      <button
        type="button"
        onClick={() => canExpand && setOpen((o) => !o)}
        disabled={!canExpand}
        className="flex w-full items-baseline gap-2 py-0.5 text-left"
      >
        {showTimestamp && (
          <span className="w-36 shrink-0 tabular-nums text-t3">{absoluteTime(event.ts)}</span>
        )}
        <span className="w-8 shrink-0 text-meta uppercase text-t3">{KIND_LABEL[event.kind]}</span>
        <span className={cn("min-w-0 flex-1 whitespace-pre-wrap break-words", stderr ? "text-bad" : "text-t2")}>
          {event.summary}
        </span>
        {canExpand && <span className="shrink-0 text-t3">{open ? "▾" : "▸"}</span>}
      </button>
      {open && event.text && (
        <pre className="mb-2 ml-10 max-h-80 overflow-auto whitespace-pre-wrap break-words rounded-control border border-line bg-app px-2 py-2 text-t2">
          {event.text}
        </pre>
      )}
    </div>
  );
}
