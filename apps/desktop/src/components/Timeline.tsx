import { useMemo } from "react";
import { motion } from "motion/react";
import type { AgentKind, Actor, TaskEvent } from "@/generated/bindings";
import { ActorMark } from "./AgentMark";
import { relativeTime, absoluteTime } from "@/lib/format";
import { cn } from "@/lib/utils";

interface Props {
  events: TaskEvent[];
  developerAgent: AgentKind;
  reviewerAgent: AgentKind;
  selectedRevision: number | null;
  onSelectRevision: (rev: number) => void;
}

type NodeTone = "run" | "ok" | "bad" | "human" | "review" | "neutral";

interface TimelineNode {
  key: string;
  label: string;
  actor: Actor;
  agent: AgentKind | null;
  revision: number | null;
  ts: string;
  tone: NodeTone;
  glyph: string | null;
}

function classify(
  ev: TaskEvent,
  developerAgent: AgentKind,
  reviewerAgent: AgentKind
): { label: string; tone: NodeTone; glyph: string | null; agent: AgentKind | null } {
  const t = ev.eventType.toLowerCase();
  const rev = ev.revision;
  const rtag = rev != null ? ` r${rev}` : "";
  const has = (s: string) => t.includes(s);

  const failed = has("fail") || has("error") || has("timeout") || has("interrupt");
  const ok = has("success") || has("succeed") || has("pass") || has("complete") || has("done");

  if (t === "privacy:api_egress_approved") return { label: "允许 API 数据外发", tone: "human", glyph: "✓", agent: null };
  if (t === "plan:proposed") return { label: `编码计划待审批${rtag}`, tone: "human", glyph: "◉", agent: developerAgent };
  if (t === "human:plan_approve") return { label: "你批准了编码计划", tone: "human", glyph: "✓", agent: null };
  if (t === "human:plan_reject") return { label: "你驳回了编码计划", tone: "human", glyph: "↩", agent: null };
  if (t === "budget:exceeded") return { label: "预算已用完，任务暂停", tone: "bad", glyph: "!", agent: null };
  if (t === "quality:gate_failed") return { label: `质量门禁未通过${rtag}`, tone: "bad", glyph: "✕", agent: reviewerAgent };
  if (t === "quality:replayed") return { label: `固定提交质量复验完成${rtag}`, tone: "ok", glyph: "✓", agent: null };
  if (t === "delivery:change_request_opened") return { label: "已创建 PR / MR，等待 CI", tone: "run", glyph: "↗", agent: null };
  if (t === "delivery:ci_failed") return { label: "远端 CI 未通过", tone: "bad", glyph: "✕", agent: null };
  if (t === "delivery:merged") return { label: "远端变更已合并", tone: "ok", glyph: "✓", agent: null };
  if (t === "human:rollback") return { label: "你已撤销 / 回滚合并", tone: "human", glyph: "↩", agent: null };
  if (t === "user:start") return { label: "启动任务", tone: "run", glyph: null, agent: null };
  if (t === "scheduler:slot") return { label: `开始本阶段${rtag}`, tone: "run", glyph: null, agent: null };
  if (t === "scheduler:council_slot") return { label: `委员会开始审查${rtag}`, tone: "review", glyph: null, agent: reviewerAgent };
  if (t === "review:council_pass") return { label: `委员会审查通过${rtag}`, tone: "ok", glyph: "✓", agent: reviewerAgent };
  if (t === "run:succeeded") return { label: `开发完成${rtag}`, tone: "ok", glyph: "✓", agent: developerAgent };
  if (has("result:repair")) {
    const repairFailed = has("failed");
    const repairDone = has("succeeded");
    return {
      label: `${repairDone ? "结构修复完成" : repairFailed ? "结构修复失败" : "正在修复结构化结果"}${rtag}`,
      tone: repairDone ? "ok" : repairFailed ? "bad" : "run",
      glyph: repairDone ? "✓" : repairFailed ? "✕" : null,
      agent: null,
    };
  }
  if (has("provider:fallback")) return { label: `切换备用 Provider${rtag}`, tone: "human", glyph: "↪", agent: null };
  if (has("recovery:interrupted")) return { label: `后台恢复 · 已重新排队${rtag}`, tone: "human", glyph: "↻", agent: null };
  if (has("creat")) return { label: "创建", tone: "neutral", glyph: null, agent: null };
  if (has("clarif")) return { label: `需要澄清${rtag}`, tone: "human", glyph: "?", agent: developerAgent };
  if (has("develop") || has("revis")) {
    const base = has("revis") ? "返工" : "开发";
    return { label: `${base}${rtag}`, tone: failed ? "bad" : ok ? "ok" : "run", glyph: failed ? "✕" : ok ? "✓" : null, agent: developerAgent };
  }
  if (has("validat") || (has("test") && !has("request"))) {
    return { label: `验证${rtag}`, tone: failed ? "bad" : ok ? "ok" : "run", glyph: failed ? "✕" : ok ? "✓" : null, agent: null };
  }
  if (has("review")) {
    const blocked = has("block") || has("request");
    return { label: `审查${rtag}`, tone: failed || blocked ? "bad" : ok ? "ok" : "review", glyph: failed || blocked ? "✕" : ok ? "✓" : null, agent: reviewerAgent };
  }
  if (has("approv")) return { label: "已批准", tone: "human", glyph: "✓", agent: null };
  if (has("reject")) return { label: "驳回返工", tone: "human", glyph: "↩", agent: null };
  if (has("wait")) return { label: "等你批准", tone: "human", glyph: "◉", agent: null };
  if (has("conflict")) return { label: "合并冲突", tone: "human", glyph: "⚠", agent: null };
  if (has("merg")) return { label: "已合并", tone: "ok", glyph: "✓", agent: null };
  if (has("block")) return { label: "需要你处理", tone: "human", glyph: "⚠", agent: null };
  if (has("cancel")) return { label: "已取消", tone: "neutral", glyph: null, agent: null };
  if (has("guidance") || has("resume")) return { label: "补充指引", tone: "human", glyph: null, agent: null };
  return { label: ev.eventType.replace(/[_:]/g, " "), tone: "neutral", glyph: null, agent: null };
}

const TONE_TEXT: Record<NodeTone, string> = {
  run: "text-run",
  ok: "text-ok",
  bad: "text-bad",
  human: "text-human",
  review: "text-review",
  neutral: "text-t3",
};
const TONE_DOT: Record<NodeTone, string> = {
  run: "bg-run border-run",
  ok: "bg-ok border-ok",
  bad: "bg-bad border-bad",
  human: "bg-human border-human",
  review: "bg-review border-review",
  neutral: "bg-t3 border-t3",
};

const MAX_NODES = 400;

export function Timeline({ events, developerAgent, reviewerAgent, selectedRevision, onSelectRevision }: Props) {
  const nodes = useMemo<TimelineNode[]>(() => {
    const byRun = new Map<string, number>();
    const out: TimelineNode[] = [];
    for (const ev of events) {
      const c = classify(ev, developerAgent, reviewerAgent);
      if (ev.runId && byRun.has(ev.runId)) {
        const idx = byRun.get(ev.runId)!;
        const prev = out[idx];
        out[idx] = { ...prev, label: c.label, tone: c.tone, glyph: c.glyph ?? prev.glyph, ts: ev.createdAt };
        continue;
      }
      const node: TimelineNode = {
        key: String(ev.id),
        label: c.label,
        actor: ev.actor,
        agent: c.agent,
        revision: ev.revision,
        ts: ev.createdAt,
        tone: c.tone,
        glyph: c.glyph,
      };
      if (ev.runId) byRun.set(ev.runId, out.length);
      out.push(node);
    }
    return out;
  }, [events, developerAgent, reviewerAgent]);

  const shown = nodes.length > MAX_NODES ? nodes.slice(nodes.length - MAX_NODES) : nodes;

  return (
    <div className="py-2">
      {nodes.length > MAX_NODES && (
        <div className="px-2 pb-2 text-center text-[12px] text-t3">
          已省略更早的 {nodes.length - MAX_NODES} 个节点
        </div>
      )}
      <ol className="list-none">
        {shown.map((n, i) => {
          const active = n.revision != null && n.revision === selectedRevision;
          const clickable = n.revision != null;
          return (
            <motion.li
              key={n.key}
              className="flex min-h-10 gap-2"
              initial={{ opacity: 0, x: -6 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ delay: Math.min(i * 0.015, 0.15), duration: 0.2 }}
            >
              <span className="relative flex w-4 shrink-0 justify-center" aria-hidden>
                {i > 0 && <span className="absolute left-1/2 top-0 h-3.5 w-px -translate-x-1/2 bg-line" />}
                <span
                  className={cn(
                    "absolute top-2 grid size-3 place-items-center rounded-full border-2",
                    TONE_DOT[n.tone]
                  )}
                >
                  {n.glyph && <span className="text-[8px] font-bold leading-none text-white">{n.glyph}</span>}
                </span>
                {i < shown.length - 1 && <span className="absolute left-1/2 top-3.5 bottom-0 w-px -translate-x-1/2 bg-line" />}
              </span>
              <button
                type="button"
                disabled={!clickable}
                onClick={() => clickable && onSelectRevision(n.revision!)}
                title={absoluteTime(n.ts)}
                className={cn(
                  "flex flex-1 items-center justify-between gap-2 rounded-md p-2 text-left transition-colors",
                  clickable && "hover:bg-raised",
                  active && "bg-raised ring-1 ring-line"
                )}
              >
                <span className={cn("flex min-w-0 items-center gap-1.5 text-[13px]", TONE_TEXT[n.tone])}>
                  {n.actor !== "system" && <ActorMark actor={n.actor} agent={n.agent} />}
                  <span className="truncate">{n.label}</span>
                </span>
                <span className="shrink-0 text-[11px] text-t3">{relativeTime(n.ts)}</span>
              </button>
            </motion.li>
          );
        })}
      </ol>
    </div>
  );
}
