import { useEffect } from "react";
import { useDiff } from "@/hooks/useTaskData";
import { useUiStore } from "@/stores/uiStore";
import { DiffPanel } from "@/components/DiffPanel";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { EmptyState } from "@/components/EmptyState";

export function DiffTab({ taskId, revision }: { taskId: string; revision: number }) {
  const diff = useDiff(taskId, revision);
  const diffJump = useUiStore((s) => s.diffJump);
  const clearDiffJump = useUiStore((s) => s.clearDiffJump);

  const jump = diffJump && diffJump.taskId === taskId ? diffJump : null;

  // Consume the jump once the diff is available.
  useEffect(() => {
    if (jump && diff.data) {
      const t = setTimeout(() => clearDiffJump(), 400);
      return () => clearTimeout(t);
    }
  }, [jump, diff.data, clearDiffJump]);

  if (revision < 1) {
    return <EmptyState title="这个 revision 还没有可查看的改动" />;
  }
  if (diff.isLoading) return <div style={{ padding: 16 }}><SkeletonRows rows={6} /></div>;
  if (diff.isError) return <ErrorState error={diff.error} onRetry={() => diff.refetch()} />;
  if (!diff.data) return <EmptyState title="没有 diff 数据" />;

  return <DiffPanel diff={diff.data} jumpFile={jump?.file ?? null} jumpLine={jump?.line ?? null} />;
}
