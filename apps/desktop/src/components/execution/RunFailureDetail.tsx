import type { RunFailureSummary } from "@/lib/execution/runFailure";
import { roleLabel } from "@/copy/permission";
import { agentLabel } from "@/copy/agents";

/**
 * §16 异常退出结构化详情。把「哪个 Agent、以什么方式退出、退出码」摊开给用户，并说清楚
 * 项目当前的安全状态。只呈现后台真实持久化的字段（agent_runs.status / exit_code）——
 * 本轮改动在总失败时已回滚，因此如实说「项目未受影响」，不谎称保存了在改文件（对照 §40）。
 */
export function RunFailureDetail({ failure }: { failure: RunFailureSummary }) {
  const who = `${roleLabel(failure.role)} Agent（${agentLabel(failure.agent)}）`;
  const how =
    failure.kind === "timeout"
      ? "运行超时，已被安全终止"
      : failure.kind === "interrupted"
        ? "运行被中断"
        : failure.exitCode != null
          ? `异常退出（退出代码 ${failure.exitCode}）`
          : "异常退出";
  // 项目安全状态按角色如实描述：开发是可写的，失败会把本轮改动回滚；审查/规划是只读的，
  // 根本没有改动可回滚，待处理的版本原样保留。不套用统一话术以免误导（对照 §40）。
  const projectState =
    failure.role === "reviewer" || failure.role === "planner"
      ? "该阶段为只读，未改动任何项目文件；待处理的版本原样保留。运行日志已保留可查看。"
      : "本轮改动已回滚，项目保持在这一轮开始前的状态，未受影响；运行日志已保留可查看。";

  return (
    <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 rounded-section border border-line bg-app/50 px-3 py-2 text-body">
      <dt className="text-t3">Agent</dt>
      <dd className="text-t1">{who}</dd>
      <dt className="text-t3">结束方式</dt>
      <dd className="text-t1">{how}</dd>
      <dt className="text-t3">项目状态</dt>
      <dd className="text-t2">{projectState}</dd>
    </dl>
  );
}
