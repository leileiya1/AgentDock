import { useEffect, useMemo, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { Bell, Bot, Database, Laptop, Play, ShieldCheck } from "lucide-react";
import { useSettings, useUpdateSettings } from "@/hooks/useSettings";
import { useProjects } from "@/hooks/useProjects";
import { SettingsProjectSection } from "@/routes/settings/ProjectSection";
import { StorageSection } from "@/routes/settings/StorageSection";
import { EnvSection } from "@/routes/settings/EnvSection";
import { ProviderSection } from "@/routes/settings/ProviderSection";
import { ExecutionNodeSection } from "@/routes/settings/ExecutionNodeSection";
import { ExecutionPermissionSection } from "@/routes/settings/ExecutionPermissionSection";
import { PermissionRulesPanel } from "@/routes/settings/PermissionRulesPanel";
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
  "rounded-section border border-line bg-panel/60 p-4";
export const sectionH = "mb-3 text-section font-semibold";
export const actionsCls = "mt-3 flex justify-end gap-2";

const SETTINGS_SECTIONS = [
  { id: "environment", label: "环境", icon: Laptop },
  { id: "providers", label: "Provider", icon: Bot },
  { id: "execution", label: "执行", icon: Play },
  { id: "permissions", label: "权限", icon: ShieldCheck },
  { id: "data", label: "安全与存储", icon: Database },
  { id: "notifications", label: "通知", icon: Bell },
] as const;
type SettingsSectionId = (typeof SETTINGS_SECTIONS)[number]["id"];

function sectionFromHash(hash: string): SettingsSectionId {
  const candidate = hash.replace(/^#/, "");
  return SETTINGS_SECTIONS.some((section) => section.id === candidate)
    ? candidate as SettingsSectionId
    : candidate === "project-settings" ? "data" : "environment";
}

export function Settings() {
  const settings = useSettings();
  const update = useUpdateSettings();
  const projects = useProjects();
  const navigate = useNavigate();
  const location = useLocation();
  const activeSection = sectionFromHash(location.hash);

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
  const runDirty = useMemo(() => {
    const current = settings.data;
    if (!current) return false;
    return JSON.stringify({
      maxConcurrentRuns: num(maxConcurrent), developerTimeoutSecs: num(devTimeout),
      reviewerTimeoutSecs: num(revTimeout), idleTimeoutSecs: num(idleTimeout), schedulerPaused,
      runWindowStart: runWindowStart || null, runWindowEnd: runWindowEnd || null,
      globalDailyCostUsd: num(globalDailyCost),
      defaultProviderMaxConcurrent: Number(defaultProviderConcurrent) || 1,
      defaultProviderRequestsPerMinute: Number(defaultProviderRpm) || 30, providerLimits,
    }) !== JSON.stringify({
      maxConcurrentRuns: current.maxConcurrentRuns, developerTimeoutSecs: current.developerTimeoutSecs,
      reviewerTimeoutSecs: current.reviewerTimeoutSecs, idleTimeoutSecs: current.idleTimeoutSecs,
      schedulerPaused: current.schedulerPaused, runWindowStart: current.runWindowStart,
      runWindowEnd: current.runWindowEnd, globalDailyCostUsd: current.globalDailyCostUsd,
      defaultProviderMaxConcurrent: current.defaultProviderMaxConcurrent,
      defaultProviderRequestsPerMinute: current.defaultProviderRequestsPerMinute,
      providerLimits: current.providerLimits,
    });
  }, [settings.data, maxConcurrent, devTimeout, revTimeout, idleTimeout, schedulerPaused, runWindowStart, runWindowEnd, globalDailyCost, defaultProviderConcurrent, defaultProviderRpm, providerLimits]);
  const notifyDirty = useMemo(() => {
    const current = settings.data?.notifications;
    return !!current && (current.enabled !== notifyEnabled || current.onAttention !== notifyAttention
      || current.onCompletion !== notifyCompletion || current.onFallback !== notifyFallback);
  }, [settings.data, notifyEnabled, notifyAttention, notifyCompletion, notifyFallback]);

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

      <div className="flex min-h-0 flex-1">
        <nav className="w-48 shrink-0 border-r border-line/70 bg-panel/45 p-3 max-sm:w-14 max-sm:px-2" aria-label="设置分区">
          {SETTINGS_SECTIONS.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              type="button"
              onClick={() => navigate(`/settings#${id}`, { replace: true })}
              aria-current={activeSection === id ? "page" : undefined}
              title={label}
              className={`mb-1 flex w-full items-center gap-2 rounded-control px-3 py-2 text-left text-body transition-colors max-sm:justify-center max-sm:px-2 ${activeSection === id ? "bg-raised font-medium text-t1" : "text-t2 hover:bg-raised/60 hover:text-t1"}`}
            >
              <Icon className="size-4 shrink-0" aria-hidden />
              <span className="max-sm:hidden">{label}</span>
            </button>
          ))}
        </nav>

        <div className="min-w-0 flex-1 overflow-y-auto px-6 py-5 max-sm:px-3">
          <div className="mx-auto flex max-w-4xl flex-col gap-4">
          {activeSection === "environment" && <EnvSection />}
          {activeSection === "providers" && <ProviderSection />}
          {activeSection === "execution" && <>
          <ExecutionNodeSection />

          <section className={sectionCls}>
            <h2 className={sectionH}>运行</h2>
            {settings.isLoading ? (
              <SkeletonRows rows={3} />
            ) : settings.isError ? (
              <ErrorState error={settings.error} onRetry={() => settings.refetch()} compact />
            ) : (
              <>
                <div className="mb-4 rounded-control border border-line bg-app p-3">
                  <Toggle label="暂停接收新的队列任务" checked={schedulerPaused} onChange={setSchedulerPaused} />
                  <p className="mt-1 text-meta text-t3">已经运行的 Agent 会安全完成；恢复后按优先级继续。</p>
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
                {runDirty && <div className={`${actionsCls} sticky bottom-0 border-t border-line bg-panel/95 pt-3`}>
                  <Button variant="primary" onClick={saveRun} disabled={update.isPending}>保存</Button>
                </div>}
              </>
            )}
          </section>
          </>}

          {activeSection === "notifications" && (
          <section className={sectionCls}>
            <h2 className={sectionH}>通知</h2>
            <div className="flex flex-col gap-2">
              <Toggle label="启用系统通知" checked={notifyEnabled} onChange={setNotifyEnabled} />
              <Toggle label="需要你介入时" checked={notifyAttention} onChange={setNotifyAttention} disabled={!notifyEnabled} />
              <Toggle label="任务完成时" checked={notifyCompletion} onChange={setNotifyCompletion} disabled={!notifyEnabled} />
              <Toggle label="Provider 降级时" checked={notifyFallback} onChange={setNotifyFallback} disabled={!notifyEnabled} />
            </div>
            {notifyDirty && <div className={`${actionsCls} sticky bottom-0 border-t border-line bg-panel/95 pt-3`}>
              <Button variant="primary" onClick={saveNotify} disabled={update.isPending}>保存</Button>
            </div>}
          </section>
          )}

          {activeSection === "permissions" && (
            <section className={sectionCls}>
              <h2 className={sectionH}>权限</h2>
              <div className="flex flex-col gap-3">
                {(projects.data ?? []).map((project) => (
                  <div key={project.id}>
                    <h3 className="mb-2 text-body font-medium text-t2">{project.name}</h3>
                    <div className="flex flex-col gap-3">
                      <ExecutionPermissionSection projectId={project.id} />
                      <PermissionRulesPanel projectId={project.id} />
                    </div>
                  </div>
                ))}
                {(projects.data?.length ?? 0) === 0 && <p className="text-body text-t3">导入项目后可管理执行权限。</p>}
              </div>
            </section>
          )}

          {activeSection === "data" && <>
          <StorageSection />
          <section id="project-settings" className={sectionCls}>
            <h2 className={sectionH}>项目设置</h2>
            {projects.data && projects.data.length > 0 ? (
              <SettingsProjectSection projects={projects.data} />
            ) : (
              <p className="text-t3">还没有导入项目。</p>
            )}
          </section>
          </>}
          </div>
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
          <div className="text-body font-medium">Provider / 账户例外</div>
          <div className="text-meta text-t3">账户是非敏感标签或 API key 环境变量名，不填写密钥。</div>
        </div>
        <Button variant="outline" size="sm" onClick={() => onChange([...value, { provider: "claude_code", account: null, maxConcurrent: 1, requestsPerMinute: 30 }])}>添加</Button>
      </div>
      <div className="mt-2 flex flex-col gap-2">
        {value.map((item, index) => (
          <div key={`${item.provider}-${index}`} className="grid grid-cols-[1.3fr_1.1fr_.7fr_.7fr_auto] items-end gap-2 rounded-control bg-app p-2">
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
    <label className={`flex cursor-pointer items-center gap-2.5 text-body ${disabled ? "opacity-50" : ""}`}>
      <Switch checked={checked} disabled={disabled} onCheckedChange={onChange} />
      <span>{label}</span>
    </label>
  );
}
