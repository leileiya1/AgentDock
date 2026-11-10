import { useMemo, useState } from "react";
import type { TaskSummary } from "@/generated/bindings";
import { PixelBot, memberShortLabel } from "@/components/pixel/PixelBot";
import { useAgentProfiles } from "@/hooks/useStudio";
import { badges, dailyPaper, type Weather } from "@/components/home/paperModel";
import { BadgeWall } from "@/components/home/BadgeWall";
import { cn } from "@/lib/utils";
import { OPEN_WEEKLY } from "@/components/weekly/WeeklyReportDialog";

const KEY = "agentflow.dailyPaper.folded";
function readFolded(): boolean {
  try {
    return localStorage.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

/** 像素天气：晴 / 多云 / 小雨 / 星空，颜色全用主题 token。 */
function WeatherIcon({ weather }: { weather: Weather }) {
  const px = (x: number, y: number, c: string, w = 1, h = 1) => <rect key={`${x}-${y}-${c}`} x={x} y={y} width={w} height={h} style={{ fill: `var(${c})` }} />;
  const sun = [px(4, 4, "--color-bot-kimi", 4, 4), px(5, 2, "--color-bot-kimi", 2, 1), px(5, 9, "--color-bot-kimi", 2, 1), px(2, 5, "--color-bot-kimi", 1, 2), px(9, 5, "--color-bot-kimi", 1, 2)];
  const cloud = [px(2, 5, "--color-t3", 9, 4), px(4, 3, "--color-t3", 4, 2), px(3, 6, "--color-studio-day-deep", 7, 2), px(5, 4, "--color-studio-day-deep", 2, 2)];
  const rain = [px(3, 9, "--color-bot-gemini"), px(6, 10, "--color-bot-gemini"), px(9, 9, "--color-bot-gemini")];
  const stars = [px(2, 2, "--color-bot-kimi"), px(8, 3, "--color-bot-kimi"), px(5, 6, "--color-bot-kimi"), px(9, 8, "--color-bot-kimi"), px(3, 4, "--color-bot-kimi-light")];
  const moon = [px(3, 6, "--color-bot-kimi-light", 3, 4), px(2, 7, "--color-bot-kimi-light", 1, 2)];
  const art = weather === "sunny" ? sun : weather === "cloudy" ? [...sun.slice(0, 2), ...cloud] : weather === "rainy" ? [...cloud, ...rain] : [...stars, ...moon];
  return (
    <svg viewBox="0 0 12 12" className="pixelated size-10 shrink-0" aria-hidden>
      {art}
    </svg>
  );
}

const TONE = { good: "text-ok", human: "text-status-human", info: "text-t1" } as const;

/** 首页顶部的「工作室小报」：今天的天气（=项目健康）、三条头条、本周之星，加一面成就墙。 */
export function DailyPaper({ tasks, failing, onOpenTask }: { tasks: TaskSummary[]; failing: number; onOpenTask: (taskId: string) => void }) {
  const [folded, setFolded] = useState(readFolded);
  const profiles = useAgentProfiles();
  const now = new Date();
  const paper = useMemo(() => dailyPaper(tasks, new Date(), failing), [tasks, failing]);
  const wall = useMemo(() => badges(tasks, new Date()), [tasks]);
  const date = now.toLocaleDateString("zh-CN", { month: "long", day: "numeric", weekday: "long" });
  const star = paper.star;
  const starProfile = star ? profiles.data?.find((p) => p.agent === star.agent) : undefined;
  const toggle = () => {
    setFolded((f) => {
      try {
        localStorage.setItem(KEY, f ? "0" : "1");
      } catch {
        /* per-viewer convenience only */
      }
      return !f;
    });
  };

  return (
    <section aria-label="工作室小报" className="overflow-hidden rounded-card bg-raised shadow-press ring-1 ring-line">
      <header className="flex items-center gap-3 border-b-4 border-double border-line-strong/60 px-4 py-2.5">
        <WeatherIcon weather={paper.weather} />
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-2">
            <span className="font-pixel text-body tracking-wider text-t1">AGENTFLOW DAILY</span>
            <span className="text-micro font-bold text-t3">工作室小报 · {date}</span>
          </div>
          <div className="truncate text-meta text-t2">{paper.forecast}</div>
        </div>
        <button
          type="button"
          onClick={() => window.dispatchEvent(new Event(OPEN_WEEKLY))}
          className={cn("rounded-row px-2 py-1 text-micro font-bold hover:bg-app hover:text-t1", now.getDay() === 5 ? "bg-[var(--color-bot-kimi-tint)] text-caution" : "text-t3")}
          title="本周成绩单：合并、返工率、MVP，可复制成图片"
        >
          {now.getDay() === 5 ? "★ 周报出炉" : "周报"}
        </button>
        <button type="button" onClick={toggle} className="rounded-row px-2 py-1 text-micro font-bold text-t3 hover:bg-app hover:text-t1" aria-expanded={!folded}>
          {folded ? "展开" : "收起"}
        </button>
      </header>
      {!folded && (
        <div className="grid gap-4 px-4 py-3 md:grid-cols-[1fr_auto]">
          <div className="flex min-w-0 flex-col gap-3">
            <ol className="flex flex-col gap-1.5">
              {paper.headlines.map((h, i) => (
                <li key={h.text} className="flex min-w-0 items-baseline gap-2">
                  <span className="font-pixel text-micro text-t3">{String(i + 1).padStart(2, "0")}</span>
                  {h.taskId ? (
                    <button type="button" onClick={() => onOpenTask(h.taskId!)} className={cn("truncate text-left text-body font-bold underline-offset-4 hover:underline", TONE[h.tone])}>
                      {h.text}
                    </button>
                  ) : (
                    <span className={cn("truncate text-body font-bold", TONE[h.tone])}>{h.text}</span>
                  )}
                </li>
              ))}
            </ol>
            <BadgeWall badges={wall} />
          </div>
          <aside className="flex items-center gap-3 rounded-card bg-[var(--color-bot-kimi-tint)] px-3 py-2 md:w-48 md:flex-col md:justify-center md:text-center" aria-label="本周之星">
            {star ? (
              <>
                <PixelBot member={star.agent} mode="done" size={44} color={starProfile?.color} decorative />
                <div>
                  <div className="text-micro font-extrabold tracking-wider text-caution">★ 本周之星</div>
                  <div className="text-body font-extrabold text-t1">{starProfile?.nickname || memberShortLabel(star.agent)}</div>
                  <div className="text-micro text-t2">7 天合并了 {star.merged} 个任务</div>
                </div>
              </>
            ) : (
              <div className="text-meta text-t2">本周还没有合并，<br />之星虚位以待</div>
            )}
          </aside>
        </div>
      )}
    </section>
  );
}
