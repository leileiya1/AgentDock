import { useState } from "react";
import { CheckCircle2, Copy, Download, ShieldCheck } from "lucide-react";
import type { AuditExportResult, TaskSummary } from "@/generated/bindings";
import { Dialog } from "@/components/Dialog";
import { Button } from "@/components/ui/button";
import { useAuditExport } from "@/hooks/useAuditExport";
import { errorLine } from "@/copy/errors";
import { formatBytes, taskCode } from "@/lib/format";
import { toast } from "@/stores/toastStore";

interface Props {
  open: boolean;
  onClose: () => void;
  projectId: string;
  tasks: TaskSummary[];
}

export function AuditExportDialog({ open, onClose, projectId, tasks }: Props) {
  const [target, setTarget] = useState("");
  const [result, setResult] = useState<AuditExportResult | null>(null);
  const exportAudit = useAuditExport(projectId);

  const runExport = async () => {
    try {
      const next = await exportAudit.mutateAsync(target || null);
      setResult(next);
      toast.info("审计记录已安全导出");
    } catch (error) {
      toast.error(errorLine(error));
    }
  };

  const copyPath = async () => {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result.path);
      toast.info("保存位置已复制");
    } catch {
      toast.error("无法复制路径，请在下方手动选择");
    }
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="导出审计记录"
      width={620}
      onConfirmKey={runExport}
      footer={
        <>
          <Button variant="outline" onClick={onClose}>关闭</Button>
          <Button variant="primary" onClick={runExport} disabled={exportAudit.isPending}>
            <Download className="size-4" />
            {exportAudit.isPending ? "导出中…" : result ? "重新导出" : "开始导出"}
          </Button>
        </>
      }
    >
      <div className="rounded-[var(--radius-control)] border border-ok/30 bg-ok/5 p-3">
        <div className="flex items-center gap-2 text-[13px] font-semibold text-t1">
          <ShieldCheck className="size-4 text-ok" /> 本地 JSONL · 已应用秘密字段脱敏
        </div>
        <p className="mt-1 text-[12px] leading-5 text-t3">
          仅导出所选项目或任务的事件。全局事件只有明确绑定当前项目时才会包含，避免混入其他项目数据。
        </p>
      </div>

      <label className="mt-4 block text-[12px] font-semibold text-t2" htmlFor="audit-export-target">
        导出范围
      </label>
      <select
        id="audit-export-target"
        value={target}
        onChange={(event) => {
          setTarget(event.target.value);
          setResult(null);
        }}
        className="mt-1.5 h-9 w-full rounded-[var(--radius-control)] border border-line bg-app px-3 text-[13px] text-t1 outline-none focus:border-run"
      >
        <option value="">整个项目（包含所有任务）</option>
        {tasks.map((task) => (
          <option key={task.id} value={task.id}>
            {taskCode(task.seq)} · {task.title}
          </option>
        ))}
      </select>

      {result && <AuditExportSummary result={result} onCopyPath={copyPath} />}
    </Dialog>
  );
}

export function AuditExportSummary({
  result,
  onCopyPath,
}: {
  result: AuditExportResult;
  onCopyPath?: () => void;
}) {
  return (
    <div className="mt-4 rounded-[var(--radius-panel)] border border-ok/40 bg-app p-3">
      <div className="flex items-center gap-2 text-[13px] font-semibold text-ok">
        <CheckCircle2 className="size-4" /> 导出完成
      </div>
      <div className="mt-2 grid grid-cols-3 gap-2 text-[12px] text-t2">
        <span>{result.eventCount} 条事件</span>
        <span>{result.taskCount} 个任务</span>
        <span>{formatBytes(result.bytes)}</span>
      </div>
      <div className="mt-3 flex items-start gap-2">
        <code className="min-w-0 flex-1 select-all break-all rounded bg-raised px-2 py-1.5 text-[11px] text-t2">
          {result.path}
        </code>
        {onCopyPath && (
          <Button variant="outline" size="sm" onClick={onCopyPath} title="复制保存位置">
            <Copy className="size-3.5" /> 复制
          </Button>
        )}
      </div>
    </div>
  );
}
