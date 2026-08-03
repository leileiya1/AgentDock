import { AlertTriangle, CheckCircle2, GitCommitHorizontal, LoaderCircle } from "lucide-react";
import type { RollbackPreflight, RollbackStrategy } from "@/generated/bindings";
import { Dialog } from "@/components/Dialog";
import { Button } from "@/components/ui/button";
import {
  rollbackAllowed,
  rollbackBlockers,
  rollbackRecommendation,
  shortCommit,
} from "@/lib/governance/rollback";

interface Props {
  open: boolean;
  strategy: RollbackStrategy | null;
  preflight?: RollbackPreflight;
  loading: boolean;
  error: boolean;
  confirming: boolean;
  onClose: () => void;
  onRetry: () => void;
  onConfirm: () => void;
}

export function RollbackPreflightDialog({
  open,
  strategy,
  preflight,
  loading,
  error,
  confirming,
  onClose,
  onRetry,
  onConfirm,
}: Props) {
  const allowed = !!strategy && !!preflight && rollbackAllowed(preflight, strategy);
  const blockers = strategy && preflight ? rollbackBlockers(preflight, strategy) : [];
  const title = strategy === "undo" ? "撤销合并前检查" : "创建回滚提交前检查";

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={title}
      width={640}
      onConfirmKey={allowed && !confirming ? onConfirm : undefined}
      footer={
        <>
          <Button variant="outline" onClick={onClose}>取消</Button>
          <Button variant="danger" disabled={!allowed || confirming} onClick={onConfirm}>
            {confirming ? "正在执行…" : "确认执行"}
          </Button>
        </>
      }
    >
      {loading && (
        <div className="flex items-center gap-2 py-8 text-body text-t3">
          <LoaderCircle className="size-4 animate-spin" /> 正在读取最新 Git 状态…
        </div>
      )}
      {error && !loading && (
        <div className="rounded-section border border-caution/35 bg-caution-bg p-3 text-body text-t2">
          无法完成回滚预检，当前不会执行任何 Git 改动。
          <Button className="ml-2" size="sm" variant="outline" onClick={onRetry}>重试</Button>
        </div>
      )}
      {preflight && !loading && !error && (
        <div className="space-y-4 text-meta text-t2">
          <div className={`rounded-section border p-3 ${allowed ? "border-ok/35 bg-ok/5" : "border-caution/35 bg-caution-bg"}`}>
            <div className="flex items-start gap-2">
              {allowed ? <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-ok" /> : <AlertTriangle className="mt-0.5 size-4 shrink-0 text-caution" />}
              <div>
                <div className="font-medium text-t1">{allowed ? "预检通过" : "当前策略不可执行"}</div>
                <div className="mt-1 leading-relaxed">{rollbackRecommendation(preflight)}</div>
              </div>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-2">
            <Fact label="目标 / 当前分支" value={`${preflight.targetBranch} / ${preflight.currentBranch ?? "detached"}`} />
            <Fact label="工作区" value={preflight.workingTreeClean ? "干净" : "有未提交改动"} />
            <Fact label="merge / HEAD" value={`${shortCommit(preflight.mergeCommit)} / ${shortCommit(preflight.headCommit)}`} />
            <Fact label="后续提交" value={`${preflight.laterCommitCount} 个`} />
          </div>

          {blockers.length > 0 && (
            <div>
              <div className="mb-1.5 font-medium text-t1">需要先处理</div>
              <ul className="space-y-1 text-caution">{blockers.map((item) => <li key={item}>· {item}</li>)}</ul>
            </div>
          )}

          <div>
            <div className="mb-1.5 font-medium text-t1">将影响 {preflight.affectedFileCount} 个文件</div>
            {preflight.affectedFiles.length > 0 ? (
              <div className="max-h-32 overflow-y-auto rounded-control border border-line bg-app/45 p-2 font-mono text-meta">
                {preflight.affectedFiles.map((path) => <div key={path}>{path}</div>)}
                {preflight.affectedFilesTruncated && <div className="mt-1 text-t3">仅显示前 50 个文件</div>}
              </div>
            ) : <div className="text-t3">没有可展示的文件影响记录。</div>}
          </div>

          {preflight.laterCommits.length > 0 && (
            <div>
              <div className="mb-1.5 font-medium text-t1">会被保护的后续提交</div>
              <div className="space-y-1 rounded-control border border-line bg-app/45 p-2">
                {preflight.laterCommits.map((commit) => (
                  <div key={commit.sha} className="flex items-center gap-2">
                    <GitCommitHorizontal className="size-3.5 shrink-0 text-t3" />
                    <span className="font-mono text-meta">{shortCommit(commit.sha)}</span>
                    <span className="truncate">{commit.subject}</span>
                  </div>
                ))}
                {preflight.laterCommitCount > preflight.laterCommits.length && (
                  <div className="text-t3">另有 {preflight.laterCommitCount - preflight.laterCommits.length} 个提交未展开</div>
                )}
              </div>
            </div>
          )}
          <p className="text-meta leading-relaxed text-t3">
            确认时会再次读取 Git 状态；如果分支、HEAD 或工作区在弹窗打开后发生变化，执行会被拒绝。
          </p>
        </div>
      )}
    </Dialog>
  );
}

function Fact({ label, value }: { label: string; value: string }) {
  return <div className="rounded-control border border-line bg-app/45 p-2"><div className="text-meta text-t3">{label}</div><div className="mt-0.5 truncate font-medium">{value}</div></div>;
}
