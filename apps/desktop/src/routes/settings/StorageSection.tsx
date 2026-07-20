import { useState } from "react";
import { useStorageReport, useStorageCleanup, useTrash, useTaskRestore, useTrashEmpty } from "@/hooks/useStorage";
import { formatBytes, relativeTime } from "@/lib/format";
import { SkeletonRows } from "@/components/Skeleton";
import { ErrorState } from "@/components/ErrorState";
import { Dialog } from "@/components/Dialog";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { Button } from "@/components/ui/button";
import { sectionCls, sectionH, actionsCls } from "@/routes/Settings";
import { cn } from "@/lib/utils";

export function StorageSection() {
  const report = useStorageReport();
  const cleanup = useStorageCleanup();
  const trash = useTrash();
  const restore = useTaskRestore();
  const empty = useTrashEmpty();
  const [confirmEmpty, setConfirmEmpty] = useState(false);

  const r = report.data;
  const run = async (fn: () => Promise<unknown>, ok: string) => {
    try {
      await fn();
      toast.info(ok);
    } catch (e) {
      toast.error(errorLine(e));
    }
  };

  return (
    <section className={sectionCls}>
      <div className="mb-3 flex items-center justify-between">
        <h2 className={cn(sectionH, "mb-0")}>存储与隐私</h2>
        <Button variant="outline" size="sm" onClick={() => report.refetch()} disabled={report.isFetching}>刷新</Button>
      </div>

      {report.isLoading ? (
        <SkeletonRows rows={3} />
      ) : report.isError ? (
        <ErrorState error={report.error} onRetry={() => report.refetch()} compact />
      ) : r ? (
        <>
          <div className="mb-2 grid grid-cols-[repeat(auto-fill,minmax(140px,1fr))] gap-2">
            <Stat label="总占用" value={formatBytes(r.totalBytes)} />
            <Stat label="数据库" value={formatBytes(r.databaseBytes)} />
            <Stat label="任务运行数据" value={formatBytes(r.taskRuntimeBytes)} />
            <Stat label="产物 artifact" value={formatBytes(r.artifactBytes)} />
            <Stat label="日志" value={formatBytes(r.logBytes)} />
            <Stat label="缓存" value={formatBytes(r.cacheBytes)} />
            <Stat label="回收站" value={`${formatBytes(r.trashBytes)} · ${r.trashEntries} 项`} />
          </div>
          <div className="truncate font-mono text-[12px] text-t3" title={r.dataDir}>{r.dataDir}</div>
          <div className={actionsCls}>
            <Button variant="outline" onClick={() => run(() => cleanup.mutateAsync(), "已按当前策略清理")} disabled={cleanup.isPending}>
              按策略清理（日志/缓存）
            </Button>
          </div>
        </>
      ) : null}

      <div className="mt-4">
        <h3 className="mb-2 text-[13px] font-semibold text-t2">回收站</h3>
        {trash.isLoading ? (
          <SkeletonRows rows={2} />
        ) : (trash.data?.length ?? 0) === 0 ? (
          <p className="text-t3">回收站是空的。</p>
        ) : (
          <>
            <ul className="flex list-none flex-col gap-1">
              {trash.data!.map((t) => (
                <li key={t.taskId} className="flex items-center gap-3 rounded-[var(--radius-control)] border border-line px-2 py-2 text-[13px]">
                  <span className="min-w-0 flex-1 truncate">{t.title}</span>
                  <span className="text-t3">{formatBytes(t.bytes)}</span>
                  <span className="text-t3" title={`到期 ${t.purgeAfter}`}>删除于 {relativeTime(t.trashedAt)}</span>
                  <Button variant="outline" size="sm" onClick={() => run(() => restore.mutateAsync(t.taskId), "已恢复")} disabled={restore.isPending}>恢复</Button>
                </li>
              ))}
            </ul>
            <div className={actionsCls}>
              <Button variant="danger" onClick={() => setConfirmEmpty(true)}>清空回收站…</Button>
            </div>
          </>
        )}
      </div>

      <Dialog
        open={confirmEmpty}
        onClose={() => setConfirmEmpty(false)}
        title="彻底清空回收站"
        footer={
          <>
            <Button variant="outline" onClick={() => setConfirmEmpty(false)}>取消</Button>
            <Button variant="danger" disabled={empty.isPending} onClick={async () => { await run(() => empty.mutateAsync(), "回收站已清空"); setConfirmEmpty(false); }}>
              彻底删除
            </Button>
          </>
        }
      >
        <p className="text-[13px] text-t2">回收站中的任务将被永久删除，无法恢复。确定继续吗？</p>
      </Dialog>
    </section>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5 rounded-[var(--radius-control)] border border-line bg-app px-2 py-2">
      <span className="text-[12px] text-t3">{label}</span>
      <span className="font-mono text-[13px]">{value}</span>
    </div>
  );
}
