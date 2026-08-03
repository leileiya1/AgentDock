import { CheckCircle2, Clock, XCircle } from "lucide-react";
import type { TestReport, TestStepReport } from "@/lib/execution/testReport";
import { stepOutcome } from "@/lib/execution/testReport";

function durationText(ms: number): string {
  if (ms < 1000) return `${ms} ms`;
  return `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)} 秒`;
}

/**
 * §18/§19 构建与测试验证结果。逐步展示通过/失败/超时（三者刻意区分——超时不是断言失败），
 * 失败步骤附带主要错误输出，让用户一眼看懂哪一步、为什么没过。计数按「步骤」而非单条用例，
 * 因为编排层只按步骤记录退出码，不解析各测试框架的逐条结果。
 */
export function ValidationReportCard({ report }: { report: TestReport }) {
  const failing = report.steps.filter((s) => stepOutcome(s) !== "passed");
  // Surface the first failing step that actually produced error output; a timeout step has an empty
  // stderr, so preferring it would hide the real assertion failure from a later step.
  const primaryError = failing.find((s) => s.stderrTail.trim());

  return (
    <div className="rounded-section border border-line bg-panel p-3">
      <div className="mb-2 flex items-center gap-2 text-body">
        {report.passed ? (
          <span className="flex items-center gap-1.5 font-semibold text-ok">
            <CheckCircle2 className="size-4" aria-hidden /> 验证通过
          </span>
        ) : (
          <span className="flex items-center gap-1.5 font-semibold text-bad">
            <XCircle className="size-4" aria-hidden /> 验证未通过
          </span>
        )}
        <span className="text-t3">
          {report.steps.length} 步 · {report.steps.length - failing.length} 通过 · {failing.length} 失败
        </span>
      </div>

      <ul className="flex flex-col gap-1">
        {report.steps.map((step, i) => (
          <StepRow key={`${step.name}-${i}`} step={step} />
        ))}
      </ul>

      {primaryError && (
        <div className="mt-2">
          <div className="mb-1 text-meta text-t3">主要错误（{primaryError.name}）</div>
          <pre className="max-h-40 overflow-auto rounded-control bg-app/70 px-3 py-2 font-mono text-meta leading-relaxed text-t1">
            {primaryError.stderrTail.trim().slice(-2000)}
          </pre>
        </div>
      )}
    </div>
  );
}

function StepRow({ step }: { step: TestStepReport }) {
  const outcome = stepOutcome(step);
  const meta =
    outcome === "passed"
      ? { icon: CheckCircle2, color: "text-ok", note: durationText(step.durationMs) }
      : outcome === "timeout"
        ? { icon: Clock, color: "text-caution", note: `超时 · ${durationText(step.durationMs)}` }
        : { icon: XCircle, color: "text-bad", note: `退出码 ${step.exitCode} · ${durationText(step.durationMs)}` };
  const Icon = meta.icon;
  return (
    <li className="flex items-center gap-2 text-body">
      <Icon className={`size-3.5 shrink-0 ${meta.color}`} aria-hidden />
      <span className="min-w-0 truncate text-t1">{step.name}</span>
      <span className="ml-auto shrink-0 tabular-nums text-t3">{meta.note}</span>
    </li>
  );
}
