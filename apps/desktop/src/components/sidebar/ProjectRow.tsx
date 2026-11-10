import { NavLink } from "react-router-dom";
import { Pin, PinOff } from "lucide-react";
import type { Project } from "@/generated/bindings";
import type { ProjectKind, Tier } from "@/lib/studio/buildings";
import { HouseThumb } from "@/components/sidebar/HouseThumb";
import type { ProjectStatus } from "@/components/sidebar/sidebarModel";
import { cn } from "@/lib/utils";

const TONE: Record<ProjectStatus["tone"], string> = {
  human: "text-status-human",
  running: "text-status-running",
  danger: "text-status-danger",
  quiet: "text-t3",
  new: "text-caution",
};

interface Props {
  project: Project;
  house: { kind: ProjectKind; tier: Tier; lit: boolean; roof: string | null };
  status: ProjectStatus;
  active: boolean;
  collapsed: boolean;
  pinned?: boolean;
  onTogglePin?: () => void;
}

/** 一个项目 = 小镇里的一栋房子：缩略图 + 名字 + 一句话状态，等你处理的件数挂在门口。 */
export function ProjectRow({ project, house, status, active, collapsed, pinned, onTogglePin }: Props) {
  return (
    <div className="group/row relative">
    <NavLink
      to={`/p/${project.id}`}
      title={`${project.name} · ${status.line}`}
      aria-current={active ? "page" : undefined}
      className={cn(
        "group relative flex items-center gap-2.5 rounded-card py-1.5 transition-[background,box-shadow,transform] duration-150",
        collapsed ? "justify-center px-1" : "pl-1.5 pr-2",
        active ? "bg-raised shadow-press ring-1 ring-line" : "hover:bg-raised/70 hover:translate-x-0.5"
      )}
    >
      {pinned && !collapsed && <Pin aria-hidden className="absolute left-0.5 top-0.5 size-2.5 -rotate-45 text-action" />}
      {active && <span aria-hidden className="absolute -left-2 top-1/2 h-6 w-1 -translate-y-1/2 rounded-r-pill bg-action" />}
      <span className={cn("relative grid size-10 shrink-0 place-items-end justify-center overflow-hidden rounded-control", active ? "bg-action-bg" : "bg-app/70 group-hover:bg-app")}>
        <HouseThumb kind={house.kind} tier={house.tier} lit={house.lit} roof={house.roof} size={40} />
        {status.running && (
          <span aria-hidden className="absolute left-1 top-1 size-1.5 animate-pulse rounded-circle bg-status-running" />
        )}
      </span>
      {!collapsed && (
        <span className="min-w-0 flex-1">
          <span className={cn("block truncate text-body leading-tight", active ? "font-extrabold text-t1" : "font-semibold text-t1/85")}>{project.name}</span>
          <span className={cn("block truncate text-micro font-semibold leading-tight", TONE[status.tone])}>{status.line}</span>
        </span>
      )}
      {status.waiting > 0 && (
        <span
          className={cn(
            "grid min-w-5 place-items-center rounded-pill bg-status-human px-1 text-micro font-extrabold text-white shadow-press-action",
            collapsed && "absolute right-0 top-0"
          )}
          aria-label={`${status.waiting} 件等你处理`}
        >
          {status.waiting}
        </span>
      )}
    </NavLink>
    {!collapsed && onTogglePin && (
      <button
        type="button"
        onClick={onTogglePin}
        title={pinned ? "取消置顶" : "置顶到「常去」"}
        aria-label={pinned ? `取消置顶 ${project.name}` : `置顶 ${project.name}`}
        aria-pressed={pinned}
        className="absolute right-1.5 top-1/2 grid size-6 -translate-y-1/2 place-items-center rounded-row bg-raised text-t3 opacity-0 shadow-press ring-1 ring-line transition-opacity hover:text-action focus-visible:opacity-100 group-hover/row:opacity-100"
      >
        {pinned ? <PinOff className="size-3.5" /> : <Pin className="size-3.5" />}
      </button>
    )}
    </div>
  );
}
