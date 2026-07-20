import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useSettings, useUpdateSettings } from "@/hooks/useSettings";
import { useProjects } from "@/hooks/useProjects";
import { SettingsProjectSection } from "@/routes/settings/ProjectSection";
import { StorageSection } from "@/routes/settings/StorageSection";
import { EnvSection } from "@/routes/settings/EnvSection";
import { ProviderSection } from "@/routes/settings/ProviderSection";
import { ExecutionNodeSection } from "@/routes/settings/ExecutionNodeSection";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";

export const sectionCls =
  "rounded-[var(--radius-panel)] border border-line bg-panel/60 p-4";
export const sectionH = "mb-3 text-[15px] font-semibold";
export const actionsCls = "mt-3 flex justify-end gap-2";

export function Settings() {
  const settings = useSettings();
  const update = useUpdateSettings();
  const projects = useProjects();
  const navigate = useNavigate();

  const [maxConcurrent, setMaxConcurrent] = useState<number | "">("");
  const [devTimeout, setDevTimeout] = useState<number | "">("");
  const [revTimeout, setRevTimeout] = useState<number | "">("");
  const [idleTimeout, setIdleTimeout] = useState<number | "">("");
  const [notifyEnabled, setNotifyEnabled] = useState(true);
  const [notifyAttention, setNotifyAttention] = useState(true);
  const [notifyCompletion, setNotifyCompletion] = useState(true);
  const [notifyFallback, setNotifyFallback] = useState(true);

  useEffect(() => {
    const s = settings.data;
    if (!s) return;
    setMaxConcurrent(s.maxConcurrentRuns ?? "");
    setDevTimeout(s.developerTimeoutSecs ?? "");
    setRevTimeout(s.reviewerTimeoutSecs ?? "");
    setIdleTimeout(s.idleTimeoutSecs ?? "");
    setNotifyEnabled(s.notifications?.enabled ?? true);
    setNotifyAttention(s.notifications?.onAttention ?? true);
    setNotifyCompletion(s.notifications?.onCompletion ?? true);
    setNotifyFallback(s.notifications?.onFallback ?? true);
  }, [settings.data]);

  const num = (v: number | "") => (v === "" ? null : v);

  const saveRun = async () => {
    try {
      await update.mutateAsync({
        ...settings.data,
        maxConcurrentRuns: num(maxConcurrent),
        developerTimeoutSecs: num(devTimeout),
        reviewerTimeoutSecs: num(revTimeout),
        idleTimeoutSecs: num(idleTimeout),
      });
      toast.info("已保存");
    } catch (e) {
      toast.error(errorLine(e));
    }
  };

  const saveNotify = async () => {
    try {
      await update.mutateAsync({
        ...settings.data,
        notifications: { enabled: notifyEnabled, onAttention: notifyAttention, onCompletion: notifyCompletion, onFallback: notifyFallback },
      });
      toast.info("已保存");
    } catch (e) {
      toast.error(errorLine(e));
    }
  };

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex shrink-0 items-center justify-between border-b border-line/70 px-6 py-4">
        <h1 className="text-xl font-semibold tracking-tight">设置</h1>
        <Button variant="outline" onClick={() => navigate(-1)}>返回</Button>
      </header>

      <div className="flex-1 overflow-y-auto px-6 py-5">
        <div className="mx-auto flex max-w-3xl flex-col gap-4">
          <EnvSection />
          <ProviderSection />
          <ExecutionNodeSection />

          <section className={sectionCls}>
            <h2 className={sectionH}>运行</h2>
            {settings.isLoading ? (
              <SkeletonRows rows={3} />
            ) : settings.isError ? (
              <ErrorState error={settings.error} onRetry={() => settings.refetch()} compact />
            ) : (
              <>
                <div className="grid grid-cols-2 gap-4">
                  <NumField label="并发运行上限" value={maxConcurrent} onChange={setMaxConcurrent} />
                  <NumField label="空闲超时（秒）" value={idleTimeout} onChange={setIdleTimeout} />
                  <NumField label="开发超时（秒）" value={devTimeout} onChange={setDevTimeout} />
                  <NumField label="审查超时（秒）" value={revTimeout} onChange={setRevTimeout} />
                </div>
                <div className={actionsCls}>
                  <Button variant="primary" onClick={saveRun} disabled={update.isPending}>保存</Button>
                </div>
              </>
            )}
          </section>

          <section className={sectionCls}>
            <h2 className={sectionH}>通知</h2>
            <div className="flex flex-col gap-2">
              <Toggle label="启用系统通知" checked={notifyEnabled} onChange={setNotifyEnabled} />
              <Toggle label="需要你介入时" checked={notifyAttention} onChange={setNotifyAttention} disabled={!notifyEnabled} />
              <Toggle label="任务完成时" checked={notifyCompletion} onChange={setNotifyCompletion} disabled={!notifyEnabled} />
              <Toggle label="Provider 降级时" checked={notifyFallback} onChange={setNotifyFallback} disabled={!notifyEnabled} />
            </div>
            <div className={actionsCls}>
              <Button variant="primary" onClick={saveNotify} disabled={update.isPending}>保存</Button>
            </div>
          </section>

          <StorageSection />

          <section className={sectionCls}>
            <h2 className={sectionH}>项目设置</h2>
            {projects.data && projects.data.length > 0 ? (
              <SettingsProjectSection projects={projects.data} />
            ) : (
              <p className="text-t3">还没有导入项目。</p>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}

function NumField({ label, value, onChange }: { label: string; value: number | ""; onChange: (v: number | "") => void }) {
  return (
    <div className="flex flex-col gap-2">
      <Label>{label}</Label>
      <Input type="number" min={0} value={value} onChange={(e) => onChange(e.target.value === "" ? "" : Number(e.target.value))} placeholder="默认" />
    </div>
  );
}

export function Toggle({
  label,
  checked,
  onChange,
  disabled,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <label className={`flex cursor-pointer items-center gap-2.5 text-[13px] ${disabled ? "opacity-50" : ""}`}>
      <Switch checked={checked} disabled={disabled} onCheckedChange={onChange} />
      <span>{label}</span>
    </label>
  );
}
