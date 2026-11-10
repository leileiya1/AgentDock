import { forwardRef } from "react";
import type { TownProject } from "@/components/studio/town/drawTown";
import { PixelIcon } from "@/components/pixel/PixelIcon";
import { BUILDINGS } from "@/lib/studio/buildings";
import { townAgents } from "@/lib/studio/projectSummary";
import { cn } from "@/lib/utils";

/** 门牌样式（装修里选的）；没选用默认的蓝牌。 */
const SIGN_STYLE: Record<string, string> = {
  default: "bg-action text-white shadow-press-action",
  wood: "bg-sign-wood text-white shadow-[0_2px_0_var(--color-sign-wood-edge)]",
  neon: "bg-sign-neon text-sign-neon-glow ring-1 ring-sign-neon-glow shadow-[0_0_8px_var(--color-sign-neon-glow)]",
  stone: "bg-sign-stone text-white tracking-wide shadow-[0_2px_0_var(--color-sign-stone-edge)]",
};

interface Props {
  project: TownProject;
  onOpen: () => void;
  /** 打开装修（造型、颜色、住户）。 */
  onDecorate: () => void;
  /** 鼠标停在这栋房子上。 */
  highlighted?: boolean;
}

/** 项目门牌：名字 + 建筑类型 + 状态小标签，点名字进项目，点小齿轮装修。位置由画布每帧更新。 */
export const TownSign = forwardRef<HTMLDivElement, Props>(function TownSign({ project, onOpen, onDecorate, highlighted }, ref) {
  const s = project.summary;
  const chips: Array<{ text: string; tone: string }> = [];
  if (project.look.construction) chips.push({ text: "施工中 · 派个任务就能完工", tone: "bg-caution text-white" });
  if (s && s.waitingForYou > 0) chips.push({ text: `等你 ${s.waitingForYou}`, tone: "bg-status-human text-white" });
  if (s && s.running > 0) chips.push({ text: `在跑 ${s.running}`, tone: "bg-status-running text-white" });
  if (s && s.failing > 0) chips.push({ text: `失败 ${s.failing}`, tone: "bg-status-danger text-white" });
  if (project.look.weeds) chips.push({ text: `长草了 · ${project.look.idleDays} 天没动`, tone: "bg-status-idle text-white" });
  else if (project.look.leaves) chips.push({ text: "很久没动", tone: "bg-status-idle text-white" });
  const style = BUILDINGS[project.kind];
  const agents = townAgents(s).length;
  return (
    <div ref={ref} className="group pointer-events-auto absolute left-0 top-0 flex flex-col items-center gap-0.5 opacity-0">
      <span className="flex items-center gap-1">
        <span className="whitespace-nowrap rounded-row bg-raised/90 px-1.5 text-micro font-bold text-t2 ring-1 ring-line">{style.building}</span>
        <button
          type="button"
          onClick={onDecorate}
          title={`装修 ${project.name}：造型、颜色、住户`}
          aria-label={`装修 ${project.name}`}
          className="grid size-5 place-items-center rounded-row bg-raised/90 text-t2 opacity-70 ring-1 ring-line transition-opacity hover:text-action focus-visible:opacity-100 group-hover:opacity-100"
        >
          <PixelIcon name="gear" size={10} />
        </button>
      </span>
      <button
        type="button"
        onClick={onOpen}
        title={`${project.name} · ${style.building}（${style.label}）· ${agents} 位 Agent · 点击进入`}
        className={cn("whitespace-nowrap rounded-row px-2 py-0.5 text-meta font-extrabold transition-transform", highlighted && "scale-110 ring-2 ring-raised", SIGN_STYLE[project.house?.sign ?? ""] ?? SIGN_STYLE.default)}
      >
        {project.name}
      </button>
      {chips.length > 0 && (
        <span className="pointer-events-none flex gap-1">
          {chips.map((c) => (
            <span key={c.text} className={cn("whitespace-nowrap rounded-pill px-1.5 text-micro font-bold", c.tone)}>
              {c.text}
            </span>
          ))}
        </span>
      )}
    </div>
  );
});
