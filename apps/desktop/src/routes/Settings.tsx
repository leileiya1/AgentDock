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
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { AgentKind, ProviderDispatchLimit } from "@/generated/bindings";
import { ALL_AGENTS, agentLabel } from "@/copy/agents";

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
  const [schedulerPaused, setSchedulerPaused] = useState(false);
  const [runWindowStart, setRunWindowStart] = useState("");
  const [runWindowEnd, setRunWindowEnd] = useState("");
  const [globalDailyCost, setGlobalDailyCost] = useState<number | "">("");
  const [defaultProviderConcurrent, setDefaultProviderConcurrent] = useState<number | "">(1);
  const [defaultProviderRpm, setDefaultProviderRpm] = useState<number | "">(30);
  const [providerLimits, setProviderLimits] = useState<ProviderDispatchLimit[]>([]);
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
    setSchedulerPaused(s.schedulerPaused ?? false);
    setRunWindowStart(s.runWindowStart ?? "");
    setRunWindowEnd(s.runWindowEnd ?? "");
    setGlobalDailyCost(s.globalDailyCostUsd ?? "");
    setDefaultProviderConcurrent(s.defaultProviderMaxConcurrent ?? 1);
    setDefaultProviderRpm(s.defaultProviderRequestsPerMinute ?? 30);
    setProviderLimits(s.providerLimits ?? []);
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
        schedulerPaused,
        runWindowStart: runWindowStart || null,
        runWindowEnd: runWindowEnd || null,
        globalDailyCostUsd: num(globalDailyCost),
        defaultProviderMaxConcurrent: Number(defaultProviderConcurrent) || 1,
        defaultProviderRequestsPerMinute: Number(defaultProviderRpm) || 30,
        providerLimits,
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
                <div className="mb-4 rounded-md border border-line bg-app p-3">
                  <Toggle label="暂停接收新的队列任务" checked={schedulerPaused} onChange={setSchedulerPaused} />
                  <p className="mt-1 text-[12px] text-t3">已经运行的 Agent 会安全完成；恢复后按优先级继续。</p>
                </div>
                <div className="grid grid-cols-2 gap-4">
                  <NumField label="并发运行上限" value={maxConcurrent} onChange={setMaxConcurrent} />
                  <NumField label="空闲超时（秒）" value={idleTimeout} onChange={setIdleTimeout} />
                  <NumField label="开发超时（秒）" value={devTimeout} onChange={setDevTimeout} />
                  <NumField label="审查超时（秒）" value={revTimeout} onChange={setRevTimeout} />
                  <NumField label="单 Provider 默认并发" value={defaultProviderConcurrent} onChange={setDefaultProviderConcurrent} />
                  <NumField label="单 Provider 默认 RPM" value={defaultProviderRpm} onChange={setDefaultProviderRpm} />
                  <NumField label="全局每日费用硬上限 ($)" value={globalDailyCost} onChange={setGlobalDailyCost} />
                  <div className="flex flex-col gap-2">
                    <Label>本地运行窗口</Label>
                    <div className="flex items-center gap-2">
                      <Input type="time" value={runWindowStart} onChange={(event) => setRunWindowStart(event.target.value)} />
                      <span className="text-t3">—</span>
                      <Input type="time" value={runWindowEnd} onChange={(event) => setRunWindowEnd(event.target.value)} />
                    </div>
                  </div>
                </div>
                <ProviderLimits value={providerLimits} onChange={setProviderLimits} />
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

function ProviderLimits({ value, onChange }: { value: ProviderDispatchLimit[]; onChange: (value: ProviderDispatchLimit[]) => void }) {
  const patch = (index: number, update: Partial<ProviderDispatchLimit>) =>
    onChange(value.map((item, i) => (i === index ? { ...item, ...update } : item)));
  return (
    <div className="mt-4 border-t border-line pt-3">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-[13px] font-medium">Provider / 账户例外</div>
          <div className="text-[12px] text-t3">账户是非敏感标签或 API key 环境变量名，不填写密钥。</div>
        </div>
        <Button variant="outline" size="sm" onClick={() => onChange([...value, { provider: "claude_code", account: null, maxConcurrent: 1, requestsPerMinute: 30 }])}>添加</Button>
      </div>
      <div className="mt-2 flex flex-col gap-2">
        {value.map((item, index) => (
          <div key={`${item.provider}-${index}`} className="grid grid-cols-[1.3fr_1.1fr_.7fr_.7fr_auto] items-end gap-2 rounded-md bg-app p-2">
            <div className="flex flex-col gap-1"><Label>Provider</Label><Select value={item.provider} onValueChange={(provider) => patch(index, { provider: provider as AgentKind })}><SelectTrigger><SelectValue /></SelectTrigger><SelectContent>{ALL_AGENTS.map((agent) => <SelectItem key={agent} value={agent}>{agentLabel(agent)}</SelectItem>)}</SelectContent></Select></div>
            <div className="flex flex-col gap-1"><Label>账户标签</Label><Input value={item.account ?? ""} onChange={(event) => patch(index, { account: event.target.value || null })} placeholder="全部账户" /></div>
            <div className="flex flex-col gap-1"><Label>并发</Label><Input type="number" min={1} max={16} value={item.maxConcurrent} onChange={(event) => patch(index, { maxConcurrent: Math.max(1, Number(event.target.value) || 1) })} /></div>
            <div className="flex flex-col gap-1"><Label>RPM</Label><Input type="number" min={1} max={600} value={item.requestsPerMinute} onChange={(event) => patch(index, { requestsPerMinute: Math.max(1, Number(event.target.value) || 1) })} /></div>
            <Button variant="ghost" size="sm" onClick={() => onChange(value.filter((_, i) => i !== index))}>删除</Button>
          </div>
        ))}
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
