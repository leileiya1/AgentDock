import { useNavigate } from "react-router-dom";
import type { AgentKind } from "@/generated/bindings";
import { PixelBot, memberShortLabel } from "@/components/pixel/PixelBot";
import { buildDots, summarizeEnvironment, type DotTone } from "@/lib/envDots";
import { useOnboarding } from "@/hooks/useEnv";
import { cn } from "@/lib/utils";

const LED: Record<DotTone, string> = {
  ok: "bg-ok shadow-[0_0_4px_var(--color-ok)]",
  bad: "bg-bad shadow-[0_0_4px_var(--color-bad)]",
  idle: "bg-idle",
};

/** 底部：此刻在岗的 Agent（像素小人在晃）+ 环境指示灯面板，点面板进设置。 */
export function SidebarFooter({ collapsed, crew, colorOf }: { collapsed: boolean; crew: AgentKind[]; colorOf: (a: AgentKind) => string | null }) {
  const navigate = useNavigate();
  const onboarding = useOnboarding();
  const dots = buildDots(onboarding.data?.env, onboarding.data?.daemonRunning);
  const env = summarizeEnvironment(onboarding.data);

  return (
    <div className="flex flex-col gap-2 p-2">
      {crew.length > 0 && (
        <div className={cn("rounded-card bg-raised/60 ring-1 ring-line/70", collapsed ? "px-1 py-1.5" : "px-2.5 py-2")} aria-label="在岗的 Agent">
          {!collapsed && <div className="mb-1 text-micro font-bold text-t3">在岗 · {crew.length} 位</div>}
          <div className={cn("flex", collapsed ? "flex-col items-center gap-0.5" : "-space-x-1")}>
            {crew.map((a) => (
              <PixelBot key={a} member={a} mode="work" size={collapsed ? 22 : 24} color={colorOf(a)} title={`${memberShortLabel(a)} 正在干活`} />
            ))}
          </div>
        </div>
      )}
      <button
        type="button"
        onClick={() => navigate("/settings")}
        title={env.blocking ? `有阻塞项：${env.detail}` : "环境状态，点击进入设置"}
        aria-label={env.blocking ? "有阻塞项，点击进入设置处理" : "环境状态，点击进入设置"}
        className={cn(
          "group flex items-center gap-2 rounded-card border px-2.5 py-2 text-left transition-colors",
          collapsed && "flex-col px-1",
          env.blocking ? "border-caution/60 bg-caution-bg text-caution" : "border-line bg-raised/80 text-t2 hover:border-line-strong hover:bg-raised"
        )}
      >
        <span className={cn("grid shrink-0 gap-[3px] rounded-row bg-t1/90 p-1", collapsed ? "grid-cols-2" : "grid-cols-4")} aria-hidden>
          {dots.map((d) => (
            <span key={d.key} className={cn("size-[5px]", LED[d.tone])} title={d.label} />
          ))}
        </span>
        {!collapsed && (
          <span className="min-w-0 flex-1">
            <span className="block truncate text-meta font-bold">{env.label}</span>
            <span className="block truncate text-micro text-t3 group-hover:text-t2">设置与环境 ⌘,</span>
          </span>
        )}
      </button>
    </div>
  );
}
