import { AlertTriangle, Minus, Plus } from "lucide-react";
import type { ReactNode } from "react";
import type { ProjectConfigTrust } from "@/generated/bindings";
import { CONFIG_CHANGE_LABEL, configPathLabel, displayConfigValue } from "@/lib/governance/projectConfig";

export function ProjectConfigChangePreview({ value }: { value: ProjectConfigTrust }) {
  const highRisk = value.changes.filter((change) => change.highRisk).length;
  return (
    <div className="space-y-3 text-[12px]">
      {value.byteOnlyChange && (
        <div className="rounded-md border border-warn/35 bg-warn/5 p-3 text-t2">只检测到注释、空白或格式字节变化；权限语义没有变化，但 SHA 授权仍已失效，需要重新确认。</div>
      )}
      {value.changes.length > 0 && (
        <div className="rounded-md border border-line bg-raised p-3">
          <div className="flex items-center justify-between gap-2 font-medium text-t1">
            <span>相对上次批准的配置变化</span>
            {highRisk > 0 && <span className="inline-flex items-center gap-1 text-human"><AlertTriangle className="size-3.5" /> {highRisk} 项权限相关</span>}
          </div>
          <div className="mt-2 space-y-2">
            {value.changes.map((change) => (
              <div key={change.path} className="rounded border border-line bg-app/55 p-2">
                <div className="flex items-center gap-2">
                  <span className={change.highRisk ? "font-medium text-human" : "font-medium text-t1"}>{configPathLabel(change.path)}</span>
                  <span className="text-[10px] text-t3">{CONFIG_CHANGE_LABEL[change.kind]}</span>
                </div>
                {change.before !== null && <ValueLine icon={<Minus className="size-3" />} tone="text-human" value={change.before} />}
                {change.after !== null && <ValueLine icon={<Plus className="size-3" />} tone="text-ok" value={change.after} />}
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="rounded-md border border-line bg-raised p-3">
        <div className="font-medium text-t1">批准后将开放</div>
        <div className="mt-2 space-y-2">
          {value.validationCommands.length ? value.validationCommands.map((command) => (
            <div key={command.name} className="rounded bg-app/60 p-2">
              <div className="font-medium text-t2">验证 · {command.name} · 最长 {command.timeoutSecs} 秒</div>
              <code className="mt-1 block break-all text-[11px] text-t1">{command.argv.join(" ")}</code>
            </div>
          )) : <div className="text-t3">没有仓库验证命令</div>}
          {!!value.extraAllowedCommands.length && <Permission label="Agent 额外命令" values={value.extraAllowedCommands} />}
          {!!value.environmentAllowlist.length && <Permission label="环境变量摘要" values={value.environmentAllowlist} />}
          {!!value.externalDependencies.length && <Permission label="外部依赖快照" values={value.externalDependencies} />}
          {!!value.containerImages.length && <Permission label="容器镜像" values={value.containerImages} />}
          <div className="text-[11px] text-t3">环境锁定：{value.lockEnvironment ? "开启" : "关闭"} · 密闭复现：{value.hermetic ? "开启" : "关闭"}</div>
        </div>
      </div>
    </div>
  );
}

function ValueLine({ icon, tone, value }: { icon: ReactNode; tone: string; value: string }) {
  return <div className={`mt-1 flex items-start gap-1 font-mono text-[10px] ${tone}`}>{icon}<pre className="min-w-0 whitespace-pre-wrap break-all">{displayConfigValue(value)}</pre></div>;
}

function Permission({ label, values }: { label: string; values: string[] }) {
  return <div><span className="text-t3">{label}：</span><span className="text-t2">{values.join("、")}</span></div>;
}
