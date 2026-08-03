import { CheckCircle2, Circle, CircleX } from "lucide-react";
import type { ExecutionNodeDiagnostic } from "@/generated/bindings";
import { NODE_RECOVERY, NODE_STATUS_LABEL, NODE_STEP_LABEL } from "@/lib/governance/executionNode";

export function ExecutionNodeDiagnostics({ diagnostics }: { diagnostics: ExecutionNodeDiagnostic[] }) {
  if (!diagnostics.length) {
    return <p className="border-t border-line/60 px-3 py-2 text-meta text-t3">尚未运行分步诊断。点击右侧刷新按钮开始检查。</p>;
  }
  return (
    <div className="border-t border-line/60 px-3 py-2">
      <div className="grid gap-1.5">
        {diagnostics.map((item) => (
          <div key={item.step} className="grid grid-cols-[18px_112px_56px_1fr_auto] items-start gap-2 rounded-control px-1 py-1 text-meta">
            {item.status === "passed" ? <CheckCircle2 className="size-4 text-ok" /> : item.status === "failed" ? <CircleX className="size-4 text-bad" /> : <Circle className="size-4 text-t3" />}
            <span className="font-medium text-t1">{NODE_STEP_LABEL[item.step]}</span>
            <span className={item.status === "failed" ? "text-bad" : "text-t3"}>{NODE_STATUS_LABEL[item.status]}</span>
            <div className="min-w-0">
              <div className="text-t2">{item.summary}{item.blocking ? "" : "（提示）"}</div>
              {item.detail && <div className="mt-0.5 break-words text-t3">{item.detail}</div>}
              {item.status === "failed" && <div className="mt-0.5 text-t2">建议：{NODE_RECOVERY[item.step]}</div>}
            </div>
            <span className="text-t3">{item.durationMs ? `${item.durationMs} ms` : "—"}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
