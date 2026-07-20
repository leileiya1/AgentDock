import { useState, type ReactNode } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import type { ApiProviderSettings, ProjectSettings } from "@/generated/bindings";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

type ApiKey = "openai" | "anthropic" | "deepseek" | "grok" | "minimax" | "kimi";

const APIS: Array<{ id: ApiKey; label: string }> = [
  { id: "openai", label: "OpenAI" },
  { id: "anthropic", label: "Anthropic" },
  { id: "deepseek", label: "DeepSeek" },
  { id: "grok", label: "Grok" },
  { id: "minimax", label: "MiniMax" },
  { id: "kimi", label: "Kimi" },
];

export function ApiRuntimeSettings({ settings, onPatch }: { settings: ProjectSettings; onPatch: (patch: Partial<ProjectSettings>) => void }) {
  const [open, setOpen] = useState(false);
  const [selected, setSelected] = useState<ApiKey>("deepseek");
  const current = settings[selected] ?? {};
  const update = (patch: Partial<ApiProviderSettings>) => {
    onPatch({ [selected]: { ...current, ...patch } });
  };

  return (
    <div className="rounded-[var(--radius-panel)] border border-line bg-app p-3">
      <button type="button" className="flex w-full items-center justify-between text-left" onClick={() => setOpen((value) => !value)}>
        <div><div className="text-[13px] font-semibold">API 高级运行设置</div><div className="mt-0.5 text-[11px] text-t3">模型、Base URL 与费用预算所需的价格快照</div></div>
        {open ? <ChevronUp className="size-4 text-t3" /> : <ChevronDown className="size-4 text-t3" />}
      </button>
      {open && <div className="mt-3 flex flex-col gap-3 border-t border-line pt-3">
        <div className="flex items-end gap-2">
          <div className="flex flex-1 flex-col gap-2"><Label>Provider</Label><Select value={selected} onValueChange={(value) => setSelected(value as ApiKey)}><SelectTrigger><SelectValue /></SelectTrigger><SelectContent>{APIS.map((api) => <SelectItem key={api.id} value={api.id}>{api.label}</SelectItem>)}</SelectContent></Select></div>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <Field label="模型"><Input value={current.model ?? ""} onChange={(event) => update({ model: event.target.value })} /></Field>
          <Field label="Base URL"><Input value={current.baseUrl ?? ""} onChange={(event) => update({ baseUrl: event.target.value })} /></Field>
          <Field label="输入价格（美元 / 百万 Token）"><Input type="number" min="0" step="0.001" value={current.inputCostPerMillion ?? ""} onChange={(event) => update({ inputCostPerMillion: optionalNumber(event.target.value) })} placeholder="未配置" /></Field>
          <Field label="输出价格（美元 / 百万 Token）"><Input type="number" min="0" step="0.001" value={current.outputCostPerMillion ?? ""} onChange={(event) => update({ outputCostPerMillion: optionalNumber(event.target.value) })} placeholder="未配置" /></Field>
        </div>
        <p className="text-[11px] leading-relaxed text-t3">价格由你按当前供应商账单填写并随项目保存；输入和输出必须同时填写。AgentFlow 不内置可能过期的价格，API 返回真实费用时优先采用真实值。</p>
      </div>}
    </div>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) { return <div className="flex flex-col gap-2"><Label>{label}</Label>{children}</div>; }
function optionalNumber(value: string): number | null { return value.trim() === "" ? null : Number(value); }
