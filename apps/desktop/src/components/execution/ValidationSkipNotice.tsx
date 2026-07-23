import { useEvents } from "@/hooks/useTaskData";
import { latestValidationOutcome } from "@/lib/execution/testReport";

/**
 * §9/§18-19: warn at the approval gate when this revision's validation was skipped because the
 * project has no configured test/build command. The code reached approval WITHOUT being validated,
 * so the human must be told rather than assume the review pass means it was tested. Renders nothing
 * when validation actually ran (passed/failed) — a genuinely test-free project can still approve.
 */
export function ValidationSkipNotice({ taskId, revision }: { taskId: string; revision: number }) {
  const events = useEvents(taskId);
  if (latestValidationOutcome(events.data ?? [], revision) !== "skipped") return null;
  return (
    <div className="mb-2 rounded-[var(--radius-panel)] border border-human bg-human-bg px-3 py-2 text-[12px] leading-relaxed text-t1">
      ⚠ 本轮未运行验证：项目没有配置测试/构建命令，代码没有被自动验证过。批准前请确认这是可接受的。
    </div>
  );
}
