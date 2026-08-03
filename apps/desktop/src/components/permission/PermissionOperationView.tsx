import { FileWarning, FolderOpen, Globe, Terminal, Variable } from "lucide-react";
import type { PermissionRequest } from "@/generated/bindings";
import { pathDisplay } from "@/lib/permission/model";
import { argvHumanHint, argvLine, parseEndpoint } from "@/lib/permission/format";
import { NETWORK_KIND } from "@/copy/permission";
import { CopyText } from "@/components/CopyText";
import { cn } from "@/lib/utils";

/**
 * 授权弹窗的「会访问什么」区块 (06 §9 第 6/7/8/9 条)。命令用等宽 argv + 人话说明；
 * 路径压成项目相对路径、工作树外显式高亮；网络显示精确域名/端口/协议并明确这是
 * 「Agent 命令访问网络」而非「模型 API 外发」；环境变量只列名称。
 */

function SectionTitle({ icon, children }: { icon: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="mb-1.5 flex items-center gap-1.5 text-meta font-semibold text-t2">
      {icon}
      {children}
    </div>
  );
}

const ACCESS_LABEL = { read: "只读", write: "写入", delete: "删除" } as const;

export function PermissionOperationView({ request }: { request: PermissionRequest }) {
  const op = request.operation;
  const endpoints = op.networkDomains.map(parseEndpoint);

  return (
    <div className="flex flex-col gap-3">
      {op.argv.length > 0 && (
        <section>
          <SectionTitle icon={<Terminal className="size-3.5" aria-hidden />}>
            执行的命令 · <span className="font-normal text-t3">{argvHumanHint(op.argv)}</span>
          </SectionTitle>
          {/* 超长 argv 只在自己的容器内横向滚动，页面正文不产生横向滚动 (§9 第 23 条)。 */}
          <div className="max-w-full overflow-x-auto rounded-control border border-line bg-app px-2.5 py-2">
            <CopyText value={argvLine(op.argv)} mono className="whitespace-pre text-meta leading-relaxed text-t1">
              <code className="whitespace-pre">{argvLine(op.argv)}</code>
            </CopyText>
          </div>
        </section>
      )}

      {op.paths.length > 0 && (
        <section>
          <SectionTitle icon={<FolderOpen className="size-3.5" aria-hidden />}>访问的文件</SectionTitle>
          <ul className="flex flex-col gap-1">
            {op.paths.map((path, i) => {
              const view = pathDisplay(path, op.cwd);
              return (
                <li
                  key={`${path.path}-${i}`}
                  className={cn(
                    "flex flex-wrap items-center gap-x-2 gap-y-1 rounded-control border px-2.5 py-1.5 text-meta",
                    view.outsideWorktree ? "border-status-human/60 bg-status-human-bg" : "border-line bg-app"
                  )}
                >
                  {view.outsideWorktree && <FileWarning className="size-3.5 shrink-0 text-status-human" aria-hidden />}
                  <span className={cn("min-w-0 break-all font-mono", view.outsideWorktree ? "text-status-human" : "text-t1")}>
                    {view.display}
                  </span>
                  <span className="shrink-0 rounded bg-raised px-1.5 py-0.5 text-micro text-t3">
                    {ACCESS_LABEL[view.access]}
                  </span>
                  {view.outsideWorktree && (
                    <span className="shrink-0 text-meta font-medium text-status-human">工作树外 · 高风险</span>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      )}

      {endpoints.length > 0 && (
        <section>
          <SectionTitle icon={<Globe className="size-3.5" aria-hidden />}>{NETWORK_KIND.agentCommand.label}</SectionTitle>
          <ul className="flex flex-col gap-1">
            {endpoints.map((ep, i) => (
              <li
                key={`${ep.raw}-${i}`}
                className="flex flex-wrap items-center gap-x-2 gap-y-1 rounded-control border border-line bg-app px-2.5 py-1.5 text-meta"
              >
                <span className="min-w-0 break-all font-mono text-t1">{ep.host}</span>
                <span className="shrink-0 rounded bg-raised px-1.5 py-0.5 text-micro text-t3">
                  端口 {ep.port ?? "未指定"}
                </span>
                <span className="shrink-0 rounded bg-raised px-1.5 py-0.5 text-micro text-t3">{ep.protocol}</span>
              </li>
            ))}
          </ul>
          <p className="mt-1 text-meta text-t3">
            这是{NETWORK_KIND.agentCommand.hint}
          </p>
        </section>
      )}

      {op.environmentNames.length > 0 && (
        <section>
          <SectionTitle icon={<Variable className="size-3.5" aria-hidden />}>读取的环境变量</SectionTitle>
          <div className="flex flex-wrap gap-1">
            {op.environmentNames.map((name) => (
              <span key={name} className="rounded bg-raised px-1.5 py-0.5 font-mono text-meta text-t2">
                {name}
              </span>
            ))}
          </div>
          <p className="mt-1 text-meta text-t3">只读取名称用于匹配白名单，不包含变量值。</p>
        </section>
      )}
    </div>
  );
}
