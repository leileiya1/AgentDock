import type { ReactNode } from "react";
import type { PermissionRequest } from "@/generated/bindings";
import { pathDisplay } from "@/lib/permission/model";
import { argvLine, argvPlainHint, parseEndpoint } from "@/lib/permission/format";
import { NETWORK_KIND } from "@/copy/permission";
import { CopyText } from "@/components/CopyText";
import { PixelIcon, type PixelIconName } from "@/components/pixel/PixelIcon";
import { cn } from "@/lib/utils";

/**
 * 授权弹窗的「会碰到什么」区块 (06 §9 第 6/7/8/9 条)。一张卡片：顶部是命令（等宽 argv +
 * 一句人话 + 在哪执行），下面每样被访问的东西一行，左侧图标块说明类别，不再逐段加标题。
 * 路径压成项目相对路径、工作树外显式高亮；网络显示精确域名/端口/协议并标明这是
 * 「Agent 命令访问网络」而非「模型 API 外发」；环境变量只列名称。
 */

const ACCESS_LABEL = { read: "只读", write: "写入", delete: "删除" } as const;

/** 只显示工作树路径的最后两段，完整路径放在悬停提示里。 */
function shortCwd(cwd: string): string {
  const parts = cwd.split(/[/\\]/).filter(Boolean);
  return parts.length > 2 ? `…/${parts.slice(-2).join("/")}` : cwd;
}

function Row({
  icon,
  tone = "neutral",
  label,
  children,
}: {
  icon: PixelIconName;
  tone?: "neutral" | "human";
  label: string;
  children: ReactNode;
}) {
  return (
    <li
      className={cn("flex items-center gap-2.5 px-3 py-2 text-meta", tone === "human" && "bg-status-human-bg")}
      aria-label={label}
    >
      <span
        className={cn(
          "grid size-7 shrink-0 place-items-center rounded-control",
          tone === "human" ? "bg-status-human text-white" : "bg-status-idle-bg text-t2"
        )}
        title={label}
        aria-hidden
      >
        <PixelIcon name={icon} size={12} />
      </span>
      <div className="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1">{children}</div>
    </li>
  );
}

function Tag({ children, className, title }: { children: ReactNode; className?: string; title?: string }) {
  return (
    <span className={cn("shrink-0 rounded-row bg-app px-1.5 py-0.5 text-micro font-semibold text-t3", className)} title={title}>
      {children}
    </span>
  );
}

export function PermissionOperationView({ request }: { request: PermissionRequest }) {
  const op = request.operation;
  const endpoints = op.networkDomains.map(parseEndpoint);
  const hasRows = op.paths.length > 0 || endpoints.length > 0 || op.environmentNames.length > 0;
  if (op.argv.length === 0 && !hasRows) return null;

  return (
    <div className="overflow-hidden rounded-section border border-line bg-raised">
      {op.argv.length > 0 && (
        <section aria-label="执行的命令" className="bg-t1 px-3.5 pb-3 pt-2.5">
          <div className="mb-1.5 flex items-center gap-2 text-micro text-app/60">
            <span className="font-bold text-app/80">执行的命令 · {argvPlainHint(op.argv)}</span>
            {op.cwd && (
              <span className="ml-auto min-w-0 truncate font-mono" title={`在这里执行：${op.cwd}`}>
                在 {shortCwd(op.cwd)}
              </span>
            )}
          </div>
          {/* 超长 argv 只在自己的容器内横向滚动，页面正文不产生横向滚动 (§9 第 23 条)。 */}
          <div className="flex max-w-full items-center gap-2 overflow-x-auto">
            <span className="shrink-0 font-mono text-meta font-bold text-bot-you" aria-hidden>&gt;_</span>
            <CopyText value={argvLine(op.argv)} mono className="whitespace-pre text-body leading-relaxed text-app">
              <code className="whitespace-pre">{argvLine(op.argv)}</code>
            </CopyText>
          </div>
        </section>
      )}

      {hasRows && (
        <ul className="flex flex-col divide-y divide-line">
          {op.paths.map((path, i) => {
            const view = pathDisplay(path, op.cwd);
            return (
              <Row
                key={`${path.path}-${i}`}
                icon={view.outsideWorktree ? "alert" : "file"}
                tone={view.outsideWorktree ? "human" : "neutral"}
                label={`访问的文件：${view.display}`}
              >
                <span className={cn("min-w-0 break-all font-mono", view.outsideWorktree ? "text-status-human" : "text-t1")}>
                  {view.display}
                </span>
                <Tag className={view.access !== "read" ? "text-t1" : undefined}>{ACCESS_LABEL[view.access]}</Tag>
                {view.outsideWorktree && (
                  <span className="shrink-0 font-bold text-status-human">工作树外 · 高风险</span>
                )}
              </Row>
            );
          })}

          {endpoints.map((ep, i) => (
            <Row key={`${ep.raw}-${i}`} icon="globe" label={`${NETWORK_KIND.agentCommand.label}：${ep.raw}`}>
              <span className="min-w-0 break-all font-mono text-t1">{ep.host}</span>
              <Tag>端口 {ep.port ?? "未指定"}</Tag>
              <Tag>{ep.protocol}</Tag>
              <span className="ml-auto shrink-0 text-micro text-t3" title={NETWORK_KIND.agentCommand.hint}>
                {NETWORK_KIND.agentCommand.label}，非模型外发
              </span>
            </Row>
          ))}

          {op.environmentNames.length > 0 && (
            <Row icon="key" label="读取的环境变量">
              {op.environmentNames.map((name) => (
                <span key={name} className="rounded-row bg-app px-1.5 py-0.5 font-mono text-t1">
                  {name}
                </span>
              ))}
              <span className="ml-auto shrink-0 text-micro text-t3">只看名字，不包含变量值</span>
            </Row>
          )}
        </ul>
      )}
    </div>
  );
}
