import { useState, type ReactNode } from "react";
import { Pencil, Plus, RefreshCw, Server, Trash2 } from "lucide-react";
import type { ExecutionNode } from "@/generated/bindings";
import { useExecutionNodeMutations, useExecutionNodes } from "@/hooks/useGovernance";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { Dialog } from "@/components/Dialog";
import { ErrorState } from "@/components/ErrorState";
import { SkeletonRows } from "@/components/Skeleton";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { sectionCls, sectionH } from "@/routes/Settings";

type Draft = Pick<ExecutionNode, "id" | "name" | "host" | "port" | "username" | "workRoot" | "enabled">;

const EMPTY: Draft = { id: "", name: "", host: "", port: 22, username: "", workRoot: "", enabled: true };

export function ExecutionNodeSection() {
  const nodes = useExecutionNodes();
  const mutations = useExecutionNodeMutations();
  const [draft, setDraft] = useState<Draft | null>(null);
  const [deleting, setDeleting] = useState<ExecutionNode | null>(null);

  const save = async () => {
    if (!draft || !draft.name.trim() || !draft.host.trim() || !draft.username.trim() || !draft.workRoot.trim()) return;
    try {
      await mutations.upsert.mutateAsync({
        ...draft,
        status: "unknown",
        platform: null,
        gitVersion: null,
        problem: null,
        lastCheckedAt: null,
      });
      setDraft(null);
      toast.info("远程节点已保存；请执行连接检查");
    } catch (error) { toast.error(errorLine(error)); }
  };

  const check = async (nodeId: string) => {
    try {
      const result = await mutations.check.mutateAsync(nodeId);
      toast.info(result.status === "online" ? "远程节点连接正常" : "连接检查未通过");
    } catch (error) { toast.error(errorLine(error)); }
  };

  const remove = async () => {
    if (!deleting) return;
    try {
      await mutations.remove.mutateAsync(deleting.id);
      setDeleting(null);
      toast.info("远程节点已删除");
    } catch (error) { toast.error(errorLine(error)); }
  };

  return (
    <section className={sectionCls}>
      <div className="mb-3 flex items-start justify-between gap-4">
        <div>
          <h2 className={sectionH + " !mb-0"}>远程执行节点</h2>
          <p className="mt-0.5 text-[12px] text-t3">通过系统 SSH 配置连接；只发送固定 commit 的归档并运行项目验证，不存储密码或私钥。</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => setDraft({ ...EMPTY })}><Plus />添加节点</Button>
      </div>

      {nodes.isLoading ? <SkeletonRows rows={2} /> : nodes.isError ? (
        <ErrorState error={nodes.error} onRetry={() => nodes.refetch()} compact />
      ) : nodes.data?.length ? (
        <div className="flex flex-col gap-2">
          {nodes.data.map((node) => (
            <div key={node.id} className="flex items-center justify-between gap-3 rounded-lg border border-line bg-app/55 p-3">
              <div className="flex min-w-0 items-start gap-3">
                <div className="relative grid size-9 shrink-0 place-items-center rounded-lg border border-line bg-raised"><Server className="size-4 text-t2" /><span className={`absolute -right-0.5 -top-0.5 size-2.5 rounded-full border-2 border-panel ${statusDot(node.status)}`} /></div>
                <div className="min-w-0">
                  <div className="flex items-center gap-2 text-[13px] font-medium"><span>{node.name}</span>{!node.enabled && <span className="text-[11px] text-t3">已停用</span>}</div>
                  <div className="mt-0.5 truncate text-[11px] text-t3">{node.username}@{node.host}:{node.port} · {node.workRoot}</div>
                  <div className="mt-1 text-[11px] text-t2">{node.status === "online" ? `${node.platform ?? "远端"} · ${node.gitVersion ?? "Git 可用"}` : node.problem ?? "尚未检查连接"}</div>
                </div>
              </div>
              <div className="flex shrink-0 gap-1">
                <Button variant="ghost" size="icon" aria-label="检查连接" title="检查连接" disabled={mutations.check.isPending} onClick={() => check(node.id)}><RefreshCw className={mutations.check.isPending ? "animate-spin" : ""} /></Button>
                <Button variant="ghost" size="icon" aria-label="编辑节点" title="编辑节点" onClick={() => setDraft(pickDraft(node))}><Pencil /></Button>
                <Button variant="ghost" size="icon" aria-label="删除节点" title="删除节点" onClick={() => setDeleting(node)}><Trash2 /></Button>
              </div>
            </div>
          ))}
        </div>
      ) : <p className="rounded-lg border border-dashed border-line p-4 text-center text-[12px] text-t3">还没有远程节点，任务会在本机执行验证。</p>}

      <NodeDialog draft={draft} setDraft={setDraft} saving={mutations.upsert.isPending} onSave={save} />
      <Dialog open={!!deleting} onClose={() => setDeleting(null)} title="删除远程执行节点" footer={<><Button variant="outline" onClick={() => setDeleting(null)}>取消</Button><Button variant="danger" disabled={mutations.remove.isPending} onClick={remove}>确认删除</Button></>}>
        <p className="text-[13px] text-t2">已被历史任务引用的节点不能删除，只能停用，避免破坏审计记录。</p>
      </Dialog>
    </section>
  );
}

function NodeDialog({ draft, setDraft, saving, onSave }: { draft: Draft | null; setDraft: (value: Draft | null) => void; saving: boolean; onSave: () => void }) {
  const update = <K extends keyof Draft>(key: K, value: Draft[K]) => draft && setDraft({ ...draft, [key]: value });
  return <Dialog open={!!draft} onClose={() => setDraft(null)} title={draft?.id ? "编辑远程执行节点" : "添加远程执行节点"} onConfirmKey={onSave} width={560} footer={<><Button variant="outline" onClick={() => setDraft(null)}>取消</Button><Button variant="primary" disabled={saving || !draft?.name.trim() || !draft.host.trim() || !draft.username.trim() || !draft.workRoot.trim()} onClick={onSave}>{saving ? "保存中…" : "保存节点"}</Button></>}>
    {draft && <div className="grid grid-cols-2 gap-3">
      <Field label="节点名称"><Input value={draft.name} onChange={(event) => update("name", event.target.value)} placeholder="例如：Mac Studio" autoFocus /></Field>
      <Field label="主机"><Input value={draft.host} onChange={(event) => update("host", event.target.value)} placeholder="hostname 或 IP" /></Field>
      <Field label="SSH 用户"><Input value={draft.username} onChange={(event) => update("username", event.target.value)} placeholder="username" /></Field>
      <Field label="SSH 端口"><Input type="number" min={1} max={65535} value={draft.port} onChange={(event) => update("port", Number(event.target.value))} /></Field>
      <div className="col-span-2"><Field label="远端工作根目录"><Input value={draft.workRoot} onChange={(event) => update("workRoot", event.target.value)} placeholder="/home/user/agentflow-runs" /></Field></div>
      <label className="col-span-2 flex items-center gap-2 text-[13px] text-t2"><Switch checked={draft.enabled} onCheckedChange={(value) => update("enabled", value)} />允许新任务使用此节点</label>
    </div>}
  </Dialog>;
}

function Field({ label, children }: { label: string; children: ReactNode }) { return <div className="flex flex-col gap-2"><Label>{label}</Label>{children}</div>; }
function pickDraft(node: ExecutionNode): Draft { return { id: node.id, name: node.name, host: node.host, port: node.port, username: node.username, workRoot: node.workRoot, enabled: node.enabled }; }
function statusDot(status: ExecutionNode["status"]): string { return status === "online" ? "bg-ok" : status === "offline" ? "bg-bad" : "bg-t3"; }
