import { useMemo, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  CircleAlert,
  CircleDashed,
  LoaderCircle,
  XCircle,
  type LucideIcon,
} from "lucide-react";
import type { TaskDetail, TaskEvent, TaskStatus } from "@/generated/bindings";
import { BLOCKED_COPY } from "@/copy/blocked";
import { relativeTime, shortSha } from "@/lib/format";
import { CopyText } from "@/components/CopyText";

interface Props {
  task: TaskDetail;
  events: TaskEvent[];
}

function summariesByRevision(events: TaskEvent[]): Map<number, string> {
  const out = new Map<number, string>();
  for (const ev of events) {
    if (ev.revision == null) continue;
    const p = ev.payload as Record<string, unknown> | null;
    if (!p || typeof p !== "object") continue;
    const summary =
      (typeof p.summary === "string" && p.summary) ||
      (typeof (p.result as Record<string, unknown>)?.summary === "string" &&
        ((p.result as Record<string, unknown>).summary as string)) ||
      null;
    const type = ev.eventType.toLowerCase();
    if (summary && (type === "run:succeeded" || type.includes("develop"))) out.set(ev.revision, summary);
  }
  return out;
}

const sectionH = "mb-3 text-[13px] font-semibold uppercase tracking-wider text-t2";

interface ProgressMeta {
  label: string;
  icon: LucideIcon;
  color: string;
  spinning?: boolean;
}

const ACTIVE_PROGRESS: Partial<Record<TaskStatus, string>> = {
  READY_FOR_DEVELOPMENT: "准备启动",
  DEVELOPING: "开发中",
  VALIDATING: "验证中",
  READY_FOR_REVIEW: "等待审查",
  REVIEWING: "审查中",
  READY_FOR_REVISION: "等待返工",
  REVISING: "返工中",
  MERGING: "合并中",
};

function currentProgress(status: TaskStatus): ProgressMeta {
  const activeLabel = ACTIVE_PROGRESS[status];
  if (activeLabel) return { label: activeLabel, icon: LoaderCircle, color: "text-run", spinning: true };
  if (status === "WAITING_FOR_HUMAN_APPROVAL") {
    return { label: "等待你批准", icon: CircleAlert, color: "text-human" };
  }
  if (status === "BLOCKED" || status === "MERGE_CONFLICT") {
    return { label: status === "BLOCKED" ? "需要你处理" : "合并冲突", icon: CircleAlert, color: "text-human" };
  }
  if (status === "CANCELLED") return { label: "已取消", icon: XCircle, color: "text-t3" };
  if (status === "DRAFT") return { label: "尚未启动", icon: CircleDashed, color: "text-t3" };
  if (status === "APPROVED") return { label: "已批准", icon: CheckCircle2, color: "text-ok" };
  return { label: "已完成", icon: CheckCircle2, color: "text-ok" };
}

function RevisionProgress({ meta }: { meta: ProgressMeta }) {
  const Icon = meta.icon;
  return (
    <span className={`flex items-center gap-1 ${meta.color}`} title={meta.label}>
      <Icon className={`size-4 ${meta.spinning ? "animate-spin" : ""}`} aria-hidden />
      <span className="text-[11px] font-medium">{meta.label}</span>
    </span>
  );
}

function RevisionSummary({ text }: { text: string }) {
  const [expanded, setExpanded] = useState(false);
  const limit = 300;
  const long = text.length > limit;
  return (
    <div className="mt-2">
      <p className="whitespace-pre-wrap text-[13px] leading-relaxed text-t2">
        {expanded || !long ? text : `${text.slice(0, limit).trimEnd()}…`}
      </p>
      {long && (
        <button type="button" onClick={() => setExpanded((value) => !value)} className="mt-1 text-[12px] text-run hover:underline">
          {expanded ? "收起" : "展开详情"}
        </button>
      )}
    </div>
  );
}

export function OverviewTab({ task, events }: Props) {
  const summaries = useMemo(() => summariesByRevision(events), [events]);
  const blockedCopy = task.blockedReason ? BLOCKED_COPY[task.blockedReason] : null;
  const hasCurrentRevision = task.revisions.some((revision) => revision.revision === task.currentRevision);
  const currentMeta = currentProgress(task.status);

  return (
    <div className="mx-auto max-w-3xl overflow-y-auto px-6 py-5">
      {task.status === "BLOCKED" && blockedCopy && (
        <div className="mb-5 rounded-[var(--radius-panel)] border border-human bg-human-bg px-4 py-3">
          <div className="mb-2 flex items-center gap-2 font-semibold text-human">
            <AlertTriangle className="size-4" /> {blockedCopy.title}
          </div>
          <p className="text-[13px] text-t1">{blockedCopy.explanation}</p>
          {task.blockedDetail && blockedCopy.detailIsQuestion && (
            <blockquote className="mt-2 rounded-r-md border-l-2 border-human bg-app/60 px-3 py-2 text-[13px] text-t1">
              {task.blockedDetail}
            </blockquote>
          )}
          <p className="mt-2 text-[12px] text-t3">请在居中的处理窗口中选择下一步。</p>
        </div>
      )}

      <section className="mb-6">
        <h2 className={sectionH}>任务描述</h2>
        {task.description.trim() ? (
          <p className="whitespace-pre-wrap leading-relaxed text-t1">{task.description}</p>
        ) : (
          <p className="text-t3">（没有填写描述）</p>
        )}
      </section>

      <section className="mb-6">
        <h2 className={sectionH}>各轮进展</h2>
        <p className="mb-3 text-[12px] text-t3">
          下面是开发 Agent 每轮的自述总结——审查输入里刻意不含这些内容，避免影响独立审查。
        </p>
        <div className="flex flex-col gap-2">
          {!hasCurrentRevision && (
            <div className="rounded-[var(--radius-panel)] border border-run/30 bg-run/5 p-3">
              <div className="flex items-center gap-2">
                <span className="font-mono text-[12px]">{task.currentRevision > 0 ? `r${task.currentRevision}` : "首轮"}</span>
                <RevisionProgress meta={currentMeta} />
                <span className="ml-auto text-[11px] text-t3">{relativeTime(task.updatedAt)}</span>
              </div>
              <p className="mt-2 text-[13px] text-t2">
                {task.status === "DRAFT"
                  ? "任务已经创建，点击开始后这里会持续显示当前进展。"
                  : "任务已经启动；首个提交完成后，这里会显示本轮改动总结。"}
              </p>
            </div>
          )}
          {task.revisions
            .slice()
            .reverse()
            .map((r) => (
              <div key={r.revision} className="rounded-[var(--radius-panel)] border border-line bg-panel p-3">
                <div className="flex items-center gap-2 text-[12px]">
                  <span className="font-mono">r{r.revision}</span>
                  <RevisionProgress
                    meta={
                      r.revision === task.currentRevision
                        ? currentMeta
                        : { label: "本轮已结束", icon: CheckCircle2, color: "text-ok" }
                    }
                  />
                  {r.commitSha && <CopyText value={r.commitSha}>{shortSha(r.commitSha)}</CopyText>}
                  {r.stat && (
                    <span className="flex gap-1.5">
                      <span className="font-mono text-ok">+{r.stat.insertions}</span>
                      <span className="font-mono text-bad">−{r.stat.deletions}</span>
                      <span className="text-t3">· {r.stat.files} 文件</span>
                    </span>
                  )}
                  <span className="ml-auto text-t3">{relativeTime(r.createdAt)}</span>
                </div>
                <RevisionSummary text={summaries.get(r.revision) ?? "（本轮没有自述总结）"} />
                {r.stat && r.stat.flagged.length > 0 && (
                  <div className="mt-2 text-[12px] text-human">⚠ 触及控制面文件：{r.stat.flagged.join("、")}</div>
                )}
              </div>
            ))}
        </div>
      </section>
    </div>
  );
}
