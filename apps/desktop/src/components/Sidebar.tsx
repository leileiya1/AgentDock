import { NavLink, useNavigate, useParams } from "react-router-dom";
import { ChevronsLeft, ChevronsRight, Plus } from "lucide-react";
import { useProjects } from "@/hooks/useProjects";
import { useOnboarding } from "@/hooks/useEnv";
import { useUiStore } from "@/stores/uiStore";
import { useLayout } from "@/hooks/useBreakpoint";
import { buildDots, summarizeEnvironment, type DotTone } from "@/lib/envDots";
import { cn } from "@/lib/utils";

const DOT_COLOR: Record<DotTone, string> = { ok: "bg-ok", bad: "bg-bad", idle: "bg-idle" };

export function Sidebar() {
  const { projectId } = useParams();
  const navigate = useNavigate();
  const userCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const toggle = useUiStore((s) => s.toggleSidebar);
  const projects = useProjects();
  const onboarding = useOnboarding();
  const layout = useLayout();

  // 窄屏（含 200% 缩放）下强制收成图标栏，把宽度让给正文 (05 §8)。
  const collapsed = userCollapsed || layout === "compact";

  const dots = buildDots(onboarding.data?.env, onboarding.data?.daemonRunning);
  const environment = summarizeEnvironment(onboarding.data);
  const blocked = environment.blocking;

  return (
    <aside
      className={cn(
        "relative flex shrink-0 flex-col border-r border-line/70 bg-panel transition-[width] duration-150 motion-reduce:transition-none",
        collapsed ? "w-14" : "w-[220px]"
      )}
    >
      <div className="flex items-center gap-2 border-b border-line/70 px-3 py-3">
        <button onClick={() => navigate("/")} className="flex min-w-0 flex-1 items-center gap-2" title="AgentFlow">
          <span className="grid size-7 shrink-0 place-items-center rounded-control border border-line bg-brand/10 font-mono text-meta font-semibold text-brand">
            AF
          </span>
          {!collapsed && <span className="truncate text-sm font-semibold tracking-tight">AgentFlow</span>}
        </button>
        <button
          onClick={toggle}
          className="grid size-6 shrink-0 place-items-center rounded-control text-t3 transition-colors hover:bg-raised hover:text-t1"
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
                "group relative flex items-center gap-2 rounded-control px-2 py-2 text-t2 transition-colors hover:bg-raised hover:text-t1",
                (isActive || p.id === projectId) && "bg-raised text-t1"
              )
            }
          >
            {(p.id === projectId) && (
              <span className="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-pill bg-selection" />
            )}
            <span className="grid size-6 shrink-0 place-items-center rounded-control border border-line bg-app text-meta font-semibold">
              {p.name.slice(0, 1).toUpperCase()}
            </span>
            {!collapsed && <span className="truncate text-body">{p.name}</span>}
          </NavLink>
        ))}
        {!collapsed && (projects.data?.length ?? 0) === 0 && (
          <button
            onClick={() => navigate("/onboarding")}
            className="mt-2 flex items-center gap-2 rounded-control border border-dashed border-line px-2 py-2 text-left text-body text-t2 transition-colors hover:border-selection/60 hover:text-t1"
          >
            <Plus className="size-4" /> 导入项目
          </button>
        )}
      </nav>

      <button
        onClick={() => navigate("/settings")}
        title={blocked ? "有阻塞项，点击进入设置处理" : "环境状态，点击进入设置"}
        className={cn(
          "m-2 flex items-center gap-2 rounded-control border px-3 py-2 transition-colors",
          blocked
            ? "border-caution/60 bg-caution-bg text-caution"
            : "border-line bg-app/60 text-t2 hover:border-line-strong"
        )}
      >
        <span className="flex flex-wrap gap-[3px]">
          {dots.map((d) => (
            <span key={d.key} className={cn("size-[7px] rounded-circle", DOT_COLOR[d.tone])} title={d.label} />
          ))}
        </span>
        {!collapsed && <span className="text-meta">{environment.label}</span>}
      </button>
    </aside>
  );
}
