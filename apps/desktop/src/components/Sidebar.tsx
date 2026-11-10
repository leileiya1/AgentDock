import { useMemo } from "react";
import { NavLink, useMatch, useNavigate, useParams } from "react-router-dom";
import { ArrowDownUp, ChevronsLeft, ChevronsRight, Plus, Search } from "lucide-react";
import { useProjectSummaries, useProjects } from "@/hooks/useProjects";
import { useAgentProfiles, useProjectAppearance } from "@/hooks/useStudio";
import { useUiStore } from "@/stores/uiStore";
import { useLayout } from "@/hooks/useBreakpoint";
import { PixelBot } from "@/components/pixel/PixelBot";
import { buildingFor, lookFromAppearance } from "@/lib/studio/buildings";
import { townAgents } from "@/lib/studio/projectSummary";
import { projectLook } from "@/lib/studio/townResidents";
import { HouseThumb } from "@/components/sidebar/HouseThumb";
import { ProjectRow } from "@/components/sidebar/ProjectRow";
import { SidebarFooter } from "@/components/sidebar/SidebarFooter";
import { SORT_LABEL, arrangeProjects, greeting, onDuty, projectStatus } from "@/components/sidebar/sidebarModel";
import { usePinStore } from "@/stores/pinStore";
import { cn } from "@/lib/utils";

/**
 * 侧栏 = 小镇的街道目录：顶上是招牌和问候，接着是「项目小镇」明信片，
 * 每个项目是它在小镇里的那栋房子；底部是在岗的 Agent 和环境指示灯。
 */
export function Sidebar() {
  const { projectId } = useParams();
  const onTown = useMatch("/town");
  const navigate = useNavigate();
  const userCollapsed = useUiStore((s) => s.sidebarCollapsed);
  const toggle = useUiStore((s) => s.toggleSidebar);
  const openPalette = useUiStore((s) => s.setPaletteOpen);
  const { pinned, togglePin, sort, cycleSort } = usePinStore();
  const projects = useProjects();
  const summaries = useProjectSummaries();
  const appearance = useProjectAppearance();
  const profiles = useAgentProfiles();
  const layout = useLayout();
  // 窄屏（含 200% 缩放）下强制收成图标栏，把宽度让给正文 (05 §8)。
  const collapsed = userCollapsed || layout === "compact";
  const hello = greeting();

  const rows = useMemo(() => {
    const bySummary = new Map((summaries.data ?? []).map((s) => [s.projectId, s]));
    const byLook = new Map((appearance.data ?? []).map((a) => [a.projectId, a]));
    const now = Date.now();
    return (projects.data ?? []).map((p) => {
      const s = bySummary.get(p.id);
      const a = byLook.get(p.id);
      const b = buildingFor(p.name, a ? lookFromAppearance(a) : undefined, { agents: townAgents(s).length, tasks: s?.taskCount ?? 0 });
      const status = projectStatus(s, now);
      return { id: p.id, lastActivityAt: s?.lastActivityAt ?? null, waiting: status.waiting, running: status.running, project: p, status, house: { kind: b.kind, tier: b.tier, lit: projectLook(s, now).lit, roof: a?.roof ?? null } };
    });
  }, [projects.data, summaries.data, appearance.data]);

  const arranged = arrangeProjects(rows, pinned, sort);
  const ordered = [...arranged.pinned, ...arranged.rest];
  const waitingTotal = rows.reduce((n, r) => n + r.status.waiting, 0);
  const crew = onDuty(summaries.data);
  const colorOf = (agent: string) => profiles.data?.find((p) => p.agent === agent)?.color ?? null;

  return (
    <aside
      className={cn(
        "relative flex shrink-0 flex-col border-r border-line/70 bg-panel transition-[width] duration-200 motion-reduce:transition-none",
        "bg-[radial-gradient(var(--color-line)_1px,transparent_1.2px)] [background-size:14px_14px]",
        collapsed ? "w-[68px]" : "w-[244px]"
      )}
    >
      {/* 招牌 */}
      <div className={cn("flex items-center gap-2 px-3 pb-2 pt-3", collapsed && "flex-col px-2")}>
        <button onClick={() => navigate("/")} className="flex min-w-0 flex-1 items-center gap-2.5" title="AgentFlow">
          <span className="grid size-9 shrink-0 place-items-center rounded-card bg-raised shadow-press ring-1 ring-line">
            <PixelBot member="claude_code" mode="work" size={26} decorative />
          </span>
          {!collapsed && (
            <span className="min-w-0 text-left">
              <span className="block font-pixel text-section leading-none tracking-tight text-t1">AgentFlow</span>
              <span className="mt-1 flex items-center gap-1 text-micro font-semibold text-t3">
                <span aria-hidden className={cn("size-2 rounded-circle", hello.night ? "bg-t3 shadow-[2px_-1px_0_var(--color-panel)_inset]" : "bg-[var(--color-bot-kimi)]")} />
                {hello.text}
              </span>
            </span>
          )}
        </button>
        <button
          onClick={toggle}
          className="grid size-7 shrink-0 place-items-center rounded-control text-t3 transition-colors hover:bg-raised hover:text-t1"
          title={collapsed ? "展开" : "折叠"}
          aria-label={collapsed ? "展开侧栏" : "折叠侧栏"}
        >
          {collapsed ? <ChevronsRight className="size-4" /> : <ChevronsLeft className="size-4" />}
        </button>
      </div>

      {/* 传送门入口：⌘K 的可见版本 */}
      <div className={cn("px-2 pb-2", collapsed && "flex justify-center")}>
        <button
          type="button"
          onClick={() => openPalette(true)}
          title="传送门：搜项目、任务和动作（⌘K）"
          aria-label="打开传送门"
          className={cn(
            "flex items-center gap-2 rounded-control bg-raised/80 text-meta font-semibold text-t3 ring-1 ring-line transition-colors hover:bg-raised hover:text-t1 hover:ring-line-strong",
            collapsed ? "size-9 justify-center" : "w-full px-2.5 py-1.5"
          )}
        >
          <Search className="size-3.5 shrink-0" />
          {!collapsed && (
            <>
              <span className="flex-1 text-left">去哪儿？</span>
              <kbd className="rounded-row bg-app px-1.5 font-pixel text-micro">⌘K</kbd>
            </>
          )}
        </button>
      </div>

      <nav className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto px-2 pb-2" aria-label="项目">
        {/* 小镇明信片：标题区进小镇，下面那排房子各自进对应的项目 */}
        <div
          className={cn(
            "relative mb-2 overflow-hidden rounded-card ring-1 transition-[box-shadow,transform] duration-150 hover:-translate-y-0.5",
            onTown ? "shadow-press-action ring-action" : "shadow-press ring-line hover:ring-line-strong",
            "bg-[linear-gradient(180deg,var(--color-studio-day)_0%,var(--color-studio-day-deep)_100%)]"
          )}
        >
          <NavLink to="/town" title="项目小镇：所有项目一张地图" className="block">
            {collapsed ? (
              <span className="grid h-11 place-items-end justify-center pb-0.5">
                <HouseThumb kind="team" tier={2} lit size={30} />
              </span>
            ) : (
              <>
                <span className="flex items-baseline justify-between px-3 pt-2.5">
                  <span className="text-body font-extrabold text-t1">项目小镇</span>
                  <span className="text-micro font-bold text-t2">{rows.length} 栋 →</span>
                </span>
                <span className="block px-3 pb-1 text-micro font-semibold text-t2">
                  {waitingTotal > 0 ? <span className="text-status-human">{waitingTotal} 件事在等你</span> : "大家都在安心干活"}
                </span>
              </>
            )}
          </NavLink>
          {!collapsed && rows.length > 0 && (
            <span className="flex h-10 items-end gap-0.5 border-t border-dashed border-line-strong/50 bg-[var(--color-studio-grid-day)] px-2">
              {ordered.slice(0, 6).map((r) => (
                <button
                  key={r.project.id}
                  type="button"
                  onClick={() => navigate(`/p/${r.project.id}`)}
                  title={`${r.project.name} · ${r.status.line}`}
                  aria-label={`进入 ${r.project.name}`}
                  className={cn(
                    "relative rounded-row transition-transform duration-150 hover:-translate-y-1 hover:scale-110",
                    r.project.id === projectId && "-translate-y-0.5 drop-shadow-[0_2px_0_var(--color-action)]"
                  )}
                >
                  <HouseThumb kind={r.house.kind} tier={r.house.tier} lit={r.house.lit} roof={r.house.roof} size={34} />
                  {r.status.waiting > 0 && <span aria-hidden className="absolute right-0.5 top-0.5 size-1.5 rounded-circle bg-status-human" />}
                </button>
              ))}
            </span>
          )}
        </div>

        {arranged.pinned.length > 0 && (
          <>
            {!collapsed && <div className="px-1.5 pb-0.5 text-micro font-extrabold tracking-wider text-action">常去 · {arranged.pinned.length}</div>}
            {arranged.pinned.map((r) => (
              <ProjectRow key={r.project.id} {...r} active={r.project.id === projectId} collapsed={collapsed} pinned onTogglePin={() => togglePin(r.project.id)} />
            ))}
            <div aria-hidden className="mx-2 my-1 border-t border-dashed border-line-strong/50" />
          </>
        )}

        <div className={cn("flex items-center justify-between px-1.5 pb-0.5", collapsed && "justify-center")}>
          {!collapsed && (
            <span className="flex items-center gap-1.5">
              <span className="text-micro font-extrabold tracking-wider text-t3">街坊 · {arranged.rest.length}</span>
              <button
                type="button"
                onClick={cycleSort}
                title="切换排序：默认 → 等我的优先 → 最近活跃"
                aria-label={`排序：${SORT_LABEL[sort]}`}
                className={cn("flex items-center gap-0.5 rounded-row px-1 text-micro font-bold transition-colors hover:bg-raised hover:text-t1", sort === "default" ? "text-t3" : "text-action")}
              >
                <ArrowDownUp className="size-3" />
                {SORT_LABEL[sort]}
              </button>
            </span>
          )}
          <button
            onClick={() => navigate("/onboarding")}
            title="导入项目"
            aria-label="导入项目"
            className="grid size-6 place-items-center rounded-row text-t3 transition-colors hover:bg-raised hover:text-action"
          >
            <Plus className="size-3.5" />
          </button>
        </div>

        {arranged.rest.map((r) => (
          <ProjectRow key={r.project.id} {...r} active={r.project.id === projectId} collapsed={collapsed} onTogglePin={() => togglePin(r.project.id)} />
        ))}
        {!collapsed && rows.length === 0 && (
          <button
            onClick={() => navigate("/onboarding")}
            className="mt-1 flex items-center gap-2 rounded-card border border-dashed border-line-strong px-3 py-3 text-left text-body text-t2 transition-colors hover:border-action/60 hover:text-t1"
          >
            <Plus className="size-4" /> 导入第一个项目，盖第一栋房子
          </button>
        )}
      </nav>

      <SidebarFooter collapsed={collapsed} crew={crew} colorOf={colorOf} />
    </aside>
  );
}
