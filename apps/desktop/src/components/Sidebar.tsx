import { NavLink, useNavigate, useParams } from "react-router-dom";
import { motion } from "motion/react";
import { ChevronsLeft, ChevronsRight, Plus } from "lucide-react";
import type { EnvReport, ProviderStatus, ToolStatus } from "@/generated/bindings";
import { useProjects } from "@/hooks/useProjects";
import { useOnboarding } from "@/hooks/useEnv";
import { useUiStore } from "@/stores/uiStore";
import { cn } from "@/lib/utils";

type DotTone = "ok" | "bad" | "idle";
interface Dot { key: string; label: string; tone: DotTone; blocking: boolean }

function toolDot(key: string, label: string, s: ToolStatus, optional: boolean): Dot {
  if (s.found && s.compatible) return { key, label: `${label}：就绪`, tone: "ok", blocking: false };
  if (!s.found && optional) return { key, label: `${label}：适配器已就绪，可稍后安装`, tone: "idle", blocking: false };
  return { key, label: `${label}：${s.problem ?? "未就绪"}`, tone: "bad", blocking: !optional };
}
function providerDot(key: string, label: string, s: ProviderStatus): Dot {
  if (s.available) return { key, label: `${label}：可用`, tone: "ok", blocking: false };
  if (s.configured) return { key, label: `${label}：${s.problem ?? "凭据不可用"}`, tone: "bad", blocking: false };
  return { key, label: `${label}：未配置`, tone: "idle", blocking: false };
}
function buildDots(env: EnvReport | undefined, daemonRunning: boolean | undefined): Dot[] {
  if (!env) return [];
  return [
    { key: "daemon", label: daemonRunning ? "调度服务：运行中" : "调度服务：未运行", tone: daemonRunning ? "ok" : "bad", blocking: !daemonRunning },
    toolDot("git", "Git", env.git, false),
    toolDot("claude", "Claude Code", env.claudeCode, false),
    toolDot("codex", "Codex", env.codex, false),
    toolDot("gemini", "Gemini CLI", env.geminiCli, true),
    toolDot("qwen", "Qwen Code", env.qwenCode, true),
    providerDot("openai", "OpenAI API", env.openaiApi),
    providerDot("anthropic", "Anthropic API", env.anthropicApi),
    providerDot("deepseek", "DeepSeek API", env.deepseekApi),
  ];
}

const DOT_COLOR: Record<DotTone, string> = { ok: "bg-ok", bad: "bg-bad", idle: "bg-idle" };

export function Sidebar() {
  const { projectId } = useParams();
  const navigate = useNavigate();
  const collapsed = useUiStore((s) => s.sidebarCollapsed);
  const toggle = useUiStore((s) => s.toggleSidebar);
  const projects = useProjects();
  const onboarding = useOnboarding();

  const dots = buildDots(onboarding.data?.env, onboarding.data?.daemonRunning);
  const blocked = dots.some((d) => d.blocking);

  return (
    <motion.aside
      animate={{ width: collapsed ? 56 : 220 }}
      transition={{ type: "spring", stiffness: 400, damping: 34 }}
      className="relative flex shrink-0 flex-col border-r border-line/70 bg-panel/70 glass"
    >
      <span className="pointer-events-none absolute inset-y-0 right-0 w-px bg-gradient-to-b from-transparent via-black/[0.04] to-transparent" />

      <div className="flex items-center gap-2 border-b border-line/70 px-3 py-3">
        <button onClick={() => navigate("/")} className="flex min-w-0 flex-1 items-center gap-2" title="AgentFlow">
          <span className="grid size-7 shrink-0 place-items-center rounded-md border border-line bg-gradient-to-br from-run/25 to-panel font-mono text-[12px] font-semibold text-t1 shadow-[var(--shadow-glow-run)]">
            AF
          </span>
          {!collapsed && <span className="truncate text-sm font-semibold tracking-tight">AgentFlow</span>}
        </button>
        <button
          onClick={toggle}
          className="grid size-6 shrink-0 place-items-center rounded-md text-t3 transition-colors hover:bg-raised hover:text-t1"
          title={collapsed ? "展开" : "折叠"}
          aria-label={collapsed ? "展开侧栏" : "折叠侧栏"}
        >
          {collapsed ? <ChevronsRight className="size-4" /> : <ChevronsLeft className="size-4" />}
        </button>
      </div>

      <nav className="flex flex-1 flex-col gap-0.5 overflow-y-auto p-2" aria-label="项目">
        {projects.data?.map((p) => (
          <NavLink
            key={p.id}
            to={`/p/${p.id}`}
            title={p.name}
            className={({ isActive }) =>
              cn(
                "group relative flex items-center gap-2 rounded-md px-2 py-2 text-t2 transition-colors hover:bg-raised hover:text-t1",
                (isActive || p.id === projectId) && "bg-raised text-t1"
              )
            }
          >
            {(p.id === projectId) && (
              <motion.span
                layoutId="proj-active"
                className="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-full bg-run shadow-[var(--shadow-glow-run)]"
                transition={{ type: "spring", stiffness: 500, damping: 34 }}
              />
            )}
            <span className="grid size-6 shrink-0 place-items-center rounded-md border border-line bg-app text-[11px] font-semibold">
              {p.name.slice(0, 1).toUpperCase()}
            </span>
            {!collapsed && <span className="truncate text-[13px]">{p.name}</span>}
          </NavLink>
        ))}
        {!collapsed && (projects.data?.length ?? 0) === 0 && (
          <button
            onClick={() => navigate("/onboarding")}
            className="mt-2 flex items-center gap-2 rounded-md border border-dashed border-line px-2 py-2 text-left text-[13px] text-t2 transition-colors hover:border-run/60 hover:text-t1"
          >
            <Plus className="size-4" /> 导入项目
          </button>
        )}
      </nav>

      <button
        onClick={() => navigate("/settings")}
        title={blocked ? "有阻塞项，点击进入设置处理" : "环境状态，点击进入设置"}
        className={cn(
          "m-2 flex items-center gap-2 rounded-md border px-3 py-2 transition-colors",
          blocked
            ? "border-human/70 bg-human-bg text-human shadow-[var(--shadow-glow-human)]"
            : "border-line bg-app/60 text-t2 hover:border-line-strong"
        )}
      >
        <span className="flex flex-wrap gap-[3px]">
          {dots.map((d) => (
            <span key={d.key} className={cn("size-[7px] rounded-full", DOT_COLOR[d.tone], d.tone === "ok" && "shadow-[0_0_6px_-1px_currentColor]")} title={d.label} />
          ))}
        </span>
        {!collapsed && <span className="text-[12px]">{blocked ? "环境有阻塞项" : "环境就绪"}</span>}
      </button>
    </motion.aside>
  );
}
