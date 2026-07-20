import { useMemo, useState } from "react";
import { ChevronDown, ChevronUp, Info } from "lucide-react";
import type { EnvReport, ProviderDescriptor, ProviderStatus, ToolStatus } from "@/generated/bindings";
import { ProviderIcon } from "@/components/ProviderIcon";
import { Dialog } from "@/components/Dialog";
import { PathField } from "@/components/PathField";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  useDeleteApiCredential,
  useDeleteCliCredential,
  useInstallCli,
  useSetApiCredential,
  useSetCliCredential,
  useSetCliPath,
} from "@/hooks/useEnv";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { cn } from "@/lib/utils";

type CliId = "claude_code" | "codex" | "gemini_cli" | "qwen_code" | "grok_cli" | "kimi_cli" | "minimax_cli";
type ApiId = "openai_api" | "anthropic_api" | "deepseek_api" | "grok_api" | "minimax_api" | "kimi_api";

interface CliDefinition {
  id: CliId;
  label: string;
  field: "claudeCode" | "codex" | "geminiCli" | "qwenCode" | "grokCli" | "kimiCli" | "minimaxCli";
  packageName: string;
  apiKeyAuth?: boolean;
}

interface ApiDefinition {
  id: ApiId;
  label: string;
  field: "openaiApi" | "anthropicApi" | "deepseekApi" | "grokApi" | "minimaxApi" | "kimiApi";
}

const MAIN_CLIS: CliDefinition[] = [
  { id: "claude_code", label: "Claude Code", field: "claudeCode", packageName: "@anthropic-ai/claude-code", apiKeyAuth: true },
  { id: "codex", label: "Codex", field: "codex", packageName: "@openai/codex", apiKeyAuth: true },
  { id: "gemini_cli", label: "Gemini CLI", field: "geminiCli", packageName: "@google/gemini-cli" },
];

const EXTRA_CLIS: CliDefinition[] = [
  { id: "qwen_code", label: "Qwen Code", field: "qwenCode", packageName: "@qwen-code/qwen-code@latest" },
  { id: "grok_cli", label: "Grok Build", field: "grokCli", packageName: "@xai-official/grok" },
  { id: "kimi_cli", label: "Kimi Code", field: "kimiCli", packageName: "@moonshot-ai/kimi-code" },
  { id: "minimax_cli", label: "MiniMax CLI", field: "minimaxCli", packageName: "mmx-cli" },
];

const MAIN_APIS: ApiDefinition[] = [
  { id: "openai_api", label: "OpenAI", field: "openaiApi" },
  { id: "anthropic_api", label: "Anthropic", field: "anthropicApi" },
  { id: "deepseek_api", label: "DeepSeek", field: "deepseekApi" },
];

const EXTRA_APIS: ApiDefinition[] = [
  { id: "grok_api", label: "Grok", field: "grokApi" },
  { id: "minimax_api", label: "MiniMax", field: "minimaxApi" },
  { id: "kimi_api", label: "Kimi", field: "kimiApi" },
];

const BUILTIN_IDS = new Set<string>([...MAIN_CLIS, ...EXTRA_CLIS, ...MAIN_APIS, ...EXTRA_APIS].map((item) => item.id));

interface Props {
  env: EnvReport;
  providers?: ProviderDescriptor[];
}

/** Progressive disclosure for Providers: status and action first, diagnostics on demand. */
export function ProviderCatalog({ env, providers = [] }: Props) {
  const [showMore, setShowMore] = useState(false);
  const [installTarget, setInstallTarget] = useState<CliDefinition | null>(null);
  const [cliCredentialTarget, setCliCredentialTarget] = useState<CliDefinition | null>(null);
  const [apiTarget, setApiTarget] = useState<ApiDefinition | null>(null);
  const overrides = useMemo(
    () => new Map(providers.filter((item) => item.source === "external").map((item) => [item.id, item])),
    [providers],
  );
  const external = useMemo(
    () => providers.filter((item) => item.source === "external" && !BUILTIN_IDS.has(item.id)),
    [providers],
  );

  return (
    <>
      <ProviderGroup title="CLI">
        {MAIN_CLIS.map((item) => (
          overrides.has(item.id)
            ? <ExternalRow key={item.id} provider={overrides.get(item.id)!} />
            : <CliRow key={item.id} item={item} status={env[item.field]} onInstall={() => setInstallTarget(item)} onConfigureCredential={item.apiKeyAuth ? () => setCliCredentialTarget(item) : undefined} />
        ))}
      </ProviderGroup>

      <ProviderGroup title="API">
        {MAIN_APIS.map((item) => (
          overrides.has(item.id)
            ? <ExternalRow key={item.id} provider={overrides.get(item.id)!} />
            : <ApiRow key={item.id} item={item} status={env[item.field]} onConfigure={() => setApiTarget(item)} />
        ))}
      </ProviderGroup>

      {(EXTRA_CLIS.length + EXTRA_APIS.length + external.length > 0) && (
        <div>
          <Button variant="ghost" size="sm" className="px-1 text-t3" onClick={() => setShowMore((value) => !value)}>
            {showMore ? <ChevronUp /> : <ChevronDown />}
            更多 Provider
            <span className="rounded-full bg-raised px-1.5 text-[10px]">{EXTRA_CLIS.length + EXTRA_APIS.length + external.length}</span>
          </Button>
          {showMore && (
            <div className="mt-2 overflow-hidden rounded-[var(--radius-control)] border border-line bg-app">
              {EXTRA_CLIS.map((item) => (
                overrides.has(item.id)
                  ? <ExternalRow key={item.id} provider={overrides.get(item.id)!} />
                  : <CliRow key={item.id} item={item} status={env[item.field]} onInstall={() => setInstallTarget(item)} />
              ))}
              {EXTRA_APIS.map((item) => (
                overrides.has(item.id)
                  ? <ExternalRow key={item.id} provider={overrides.get(item.id)!} />
                  : <ApiRow key={item.id} item={item} status={env[item.field]} onConfigure={() => setApiTarget(item)} />
              ))}
              {external.map((item) => <ExternalRow key={item.id} provider={item} />)}
            </div>
          )}
        </div>
      )}

      <InstallDialog target={installTarget} onClose={() => setInstallTarget(null)} />
      <CliCredentialDialog
        target={cliCredentialTarget}
        status={cliCredentialTarget ? env[cliCredentialTarget.field] : null}
        onClose={() => setCliCredentialTarget(null)}
      />
      <ApiDialog target={apiTarget} status={apiTarget ? env[apiTarget.field] : null} onClose={() => setApiTarget(null)} />
    </>
  );
}

function ProviderGroup({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wider text-t3">{title}</div>
      <div className="overflow-hidden rounded-[var(--radius-control)] border border-line bg-app">{children}</div>
    </div>
  );
}

function StatusDot({ ready }: { ready: boolean }) {
  return <span className={cn("size-2 shrink-0 rounded-full", ready ? "bg-ok shadow-[0_0_7px_-1px_var(--color-ok)]" : "bg-bad")} />;
}

function RowShell({ icon, title, ready, statusText, actions, details }: {
  icon: string; title: string; ready: boolean; statusText: string; actions: React.ReactNode; details?: React.ReactNode;
}) {
  return (
    <div className="border-b border-line/70 px-3 py-2.5 last:border-b-0">
      <div className="flex items-center gap-3">
        <ProviderIcon provider={icon} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2"><span className="font-medium">{title}</span><StatusDot ready={ready} /></div>
          <div className={cn("mt-0.5 text-[12px]", ready ? "text-t3" : "text-bad")}>{statusText}</div>
        </div>
        <div className="flex shrink-0 items-center gap-1">{actions}</div>
      </div>
      {details}
    </div>
  );
}

function cliReady(status: ToolStatus) {
  return status.found && status.compatible && status.authenticated !== false;
}

function authMethodLabel(method: string | null) {
  const labels: Record<string, string> = {
    account: "账号登录",
    api_key: "API Key",
    oauth_token: "OAuth Token",
    access_token: "访问令牌",
  };
  return method ? labels[method] ?? method : null;
}

function CliRow({ item, status, onInstall, onConfigureCredential }: {
  item: CliDefinition;
  status: ToolStatus;
  onInstall: () => void;
  onConfigureCredential?: () => void;
}) {
  const [details, setDetails] = useState(false);
  const [path, setPath] = useState(status.path ?? "");
  const setCliPath = useSetCliPath();
  const ready = cliReady(status);
  const keychainProblem = status.authProblem?.includes("钥匙串") ?? false;
  const statusText = ready
    ? "已连接"
    : !status.found
      ? "未安装"
      : status.authenticated === false
        ? keychainProblem ? "钥匙串异常" : "需要登录"
        : "需要处理";

  const savePath = async () => {
    try {
      await setCliPath.mutateAsync({ tool: item.id, path });
      toast.info(`${item.label} 路径已保存`);
    } catch (error) {
      toast.error(errorLine(error));
    }
  };

  return (
    <RowShell
      icon={item.id}
      title={item.label}
      ready={ready}
      statusText={statusText}
      actions={
        <>
          {!status.found && <Button variant="primary" size="sm" onClick={onInstall}>安装</Button>}
          {status.found && onConfigureCredential && <Button variant="ghost" size="sm" onClick={onConfigureCredential}>认证</Button>}
          <Button variant="ghost" size="sm" onClick={() => setDetails((value) => !value)}>{details ? "收起" : "详情"}</Button>
        </>
      }
      details={details && (
        <div className="ml-[52px] mt-2 rounded-md bg-panel/70 px-3 py-2 text-[12px] text-t3">
          {status.version && <div>版本 {status.version}</div>}
          {status.authMethod && <div>认证方式 {authMethodLabel(status.authMethod)}</div>}
          {status.problem && <div className="mb-2 text-bad">{status.problem}</div>}
          {status.authProblem && <div className="mb-2 text-bad">{status.authProblem}</div>}
          <PathField value={path} onChange={setPath} onDetect={savePath} detecting={setCliPath.isPending} />
        </div>
      )}
    />
  );
}

function CliCredentialDialog({ target, status, onClose }: {
  target: CliDefinition | null;
  status: ToolStatus | null;
  onClose: () => void;
}) {
  const [key, setKey] = useState("");
  const setCredential = useSetCliCredential();
  const deleteCredential = useDeleteCliCredential();
  const save = async () => {
    if (!target) return;
    try {
      await setCredential.mutateAsync({ tool: target.id, apiKey: key });
      setKey("");
      toast.info(`${target.label} 已改用 AgentFlow 管理的 API Key`);
      onClose();
    } catch (error) {
      toast.error(errorLine(error));
    }
  };
  const remove = async () => {
    if (!target) return;
    try {
      await deleteCredential.mutateAsync({ tool: target.id, apiKey: null });
      toast.info(`${target.label} 的 AgentFlow API Key 已移除`);
      onClose();
    } catch (error) {
      toast.error(errorLine(error));
    }
  };
  const pending = setCredential.isPending || deleteCredential.isPending;
  const envKey = target?.id === "codex" ? "CODEX_API_KEY" : "ANTHROPIC_API_KEY";
  return (
    <Dialog
      open={target != null}
      onClose={onClose}
      title={`${target?.label ?? "CLI"} 认证`}
      onConfirmKey={save}
      footer={<><Button variant="danger" onClick={remove} disabled={pending}>移除 AgentFlow 密钥</Button><span className="flex-1" /><Button variant="ghost" onClick={onClose}>取消</Button><Button variant="primary" onClick={save} disabled={pending || !key.trim()}>保存 API Key</Button></>}
    >
      <p className="mb-3 text-[12px] text-t3">当前认证：{authMethodLabel(status?.authMethod ?? null) ?? "未认证"}。账号登录和 API Key 都受支持。</p>
      <label className="flex flex-col gap-2 text-[13px]">
        <span className="font-medium">{envKey}</span>
        <Input type="password" autoComplete="off" value={key} onChange={(event) => setKey(event.target.value)} placeholder="粘贴 CLI 专用 API Key" autoFocus />
      </label>
      <p className="mt-3 text-[12px] text-t3">密钥只保存到 macOS 钥匙串，并仅注入 AgentFlow 启动的 {target?.label} 进程。配置后将按 API 用量计费，并优先于该 CLI 的账号登录。</p>
    </Dialog>
  );
}

function ApiRow({ item, status, onConfigure }: { item: ApiDefinition; status: ProviderStatus; onConfigure: () => void }) {
  return (
    <RowShell
      icon={item.id}
      title={item.label}
      ready={status.available}
      statusText={status.available ? "已连接" : "未配置"}
      actions={<Button variant={status.available ? "ghost" : "outline"} size="sm" onClick={onConfigure}>配置</Button>}
    />
  );
}

function ExternalRow({ provider }: { provider: ProviderDescriptor }) {
  const [details, setDetails] = useState(false);
  return (
    <RowShell
      icon={provider.id}
      title={provider.displayName}
      ready={provider.available}
      statusText={provider.available ? "已连接 · 签名已验证" : provider.trust === "quarantined" ? "已隔离" : "不可用"}
      actions={<Button variant="ghost" size="sm" onClick={() => setDetails((value) => !value)}>{details ? "收起" : "详情"}</Button>}
      details={details && (
        <div className="ml-[52px] mt-2 rounded-md bg-panel/70 px-3 py-2 text-[12px] text-t3">
          <div>{provider.id} · 协议 v{provider.protocolVersion}</div>
          <div className="mt-1">
            {provider.executionLocation === "local" ? "本地执行" : provider.executionLocation === "remote" ? "远程执行" : "混合执行"}
            {provider.dataEgress !== "none" ? ` · 外发 ${provider.dataEgress}` : " · 不外发"}
            {(provider.permissions.networkDomains?.length ?? 0) > 0 ? ` · 网络域 ${provider.permissions.networkDomains?.join(", ")}` : ""}
          </div>
          {provider.problem && <div className="mt-1 text-bad">{provider.problem}</div>}
        </div>
      )}
    />
  );
}

function InstallDialog({ target, onClose }: { target: CliDefinition | null; onClose: () => void }) {
  const install = useInstallCli();
  const confirm = async () => {
    if (!target) return;
    try {
      await install.mutateAsync({ tool: target.id });
      toast.info(`${target.label} 已安装，请按提示完成登录`);
      onClose();
    } catch (error) {
      toast.error(errorLine(error));
    }
  };
  return (
    <Dialog
      open={target != null}
      onClose={onClose}
      title={`安装 ${target?.label ?? "CLI"}`}
      onConfirmKey={confirm}
      footer={<><Button variant="ghost" onClick={onClose}>取消</Button><Button variant="primary" onClick={confirm} disabled={install.isPending}>{install.isPending ? "安装中…" : "确认安装"}</Button></>}
    >
      <div className="flex gap-3 text-[13px] text-t2">
        <Info className="mt-0.5 size-4 shrink-0 text-run" />
        <div>
          <p>AgentFlow 将通过官方 npm 包安装到当前用户环境，完成后会自动重新检测。</p>
          <code className="mt-2 block rounded-md bg-app px-3 py-2 font-mono text-[12px] text-t1">npm install -g {target?.packageName}</code>
        </div>
      </div>
    </Dialog>
  );
}

function ApiDialog({ target, status, onClose }: { target: ApiDefinition | null; status: ProviderStatus | null; onClose: () => void }) {
  const [key, setKey] = useState("");
  const setCredential = useSetApiCredential();
  const deleteCredential = useDeleteApiCredential();
  const save = async () => {
    if (!target) return;
    try {
      await setCredential.mutateAsync({ provider: target.id, apiKey: key });
      setKey("");
      toast.info(`${target.label} 已连接`);
      onClose();
    } catch (error) {
      toast.error(errorLine(error));
    }
  };
  const remove = async () => {
    if (!target) return;
    try {
      await deleteCredential.mutateAsync({ provider: target.id, apiKey: null });
      toast.info(`${target.label} 钥匙串密钥已删除`);
      onClose();
    } catch (error) {
      toast.error(errorLine(error));
    }
  };
  const pending = setCredential.isPending || deleteCredential.isPending;
  return (
    <Dialog
      open={target != null}
      onClose={onClose}
      title={`配置 ${target?.label ?? "API"}`}
      onConfirmKey={save}
      footer={<>{status?.available && <Button variant="danger" onClick={remove} disabled={pending}>移除密钥</Button>}<span className="flex-1" /><Button variant="ghost" onClick={onClose}>取消</Button><Button variant="primary" onClick={save} disabled={pending || !key.trim()}>保存并检测</Button></>}
    >
      <label className="flex flex-col gap-2 text-[13px]">
        <span className="font-medium">API 密钥</span>
        <Input type="password" autoComplete="off" value={key} onChange={(event) => setKey(event.target.value)} placeholder="粘贴密钥" autoFocus />
      </label>
      <p className="mt-3 text-[12px] text-t3">密钥仅保存到 macOS 钥匙串，不写入项目、数据库或日志。模型和 Base URL 可在项目设置的高级选项中调整。</p>
    </Dialog>
  );
}
