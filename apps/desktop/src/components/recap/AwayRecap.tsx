import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { useProjects } from "@/hooks/useProjects";
import { useAllTasks } from "@/hooks/useAllTasks";
import { PixelBot } from "@/components/pixel/PixelBot";
import { taskCode } from "@/components/palette/paletteModel";
import { AWAY_MS, awayLabel, buildRecap, readLastSeen, writeLastSeen, type Recap, type RecapBucket } from "@/components/recap/awayModel";
import { cn } from "@/lib/utils";

const BUCKET: Record<RecapBucket, { label: string; chip: string; dot: string }> = {
  merged: { label: "已合并", chip: "bg-ok/10 text-ok", dot: "bg-ok" },
  trouble: { label: "出岔子", chip: "bg-status-danger/10 text-status-danger", dot: "bg-status-danger" },
  waiting: { label: "等你", chip: "bg-status-human/10 text-status-human", dot: "bg-status-human" },
  moving: { label: "在推进", chip: "bg-status-running/10 text-status-running", dot: "bg-status-running" },
};
const ORDER: RecapBucket[] = ["merged", "trouble", "waiting", "moving"];
const HEARTBEAT_MS = 60_000;

/**
 * 「你不在的时候」：离开 10 分钟以上再回来（重开应用或切回窗口），
 * 报童机器人送来一张小报，列出离开期间有动静的任务。只记在本机。
 */
export function AwayRecap() {
  const navigate = useNavigate();
  const projects = useProjects();
  // 这次回来之前最后一次"在场"的时间；null = 不用汇报。
  const [since, setSince] = useState<number | null>(() => readLastSeen());
  const [recap, setRecap] = useState<Recap | null>(null);
  const { tasks, key, ready } = useAllTasks(since !== null);

  // 在场心跳：窗口可见时每分钟记一次，切走时记一次，切回来时看看离开了多久。
  useEffect(() => {
    const beat = () => {
      if (document.visibilityState === "visible") writeLastSeen(Date.now());
    };
    const onVisibility = () => {
      if (document.visibilityState === "hidden") {
        writeLastSeen(Date.now());
        return;
      }
      const last = readLastSeen();
      if (last !== null && Date.now() - last >= AWAY_MS) setSince(last);
      writeLastSeen(Date.now());
    };
    beat();
    const timer = window.setInterval(beat, HEARTBEAT_MS);
    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("pagehide", beat);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("pagehide", beat);
    };
  }, []);

  // 数据齐了就出一张小报，然后不再跟着数据变。
  useEffect(() => {
    if (since === null || !ready) return;
    setRecap(buildRecap(tasks, projects.data ?? [], since, Date.now()));
    setSince(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- key stands in for tasks
  }, [since, ready, key, projects.data]);

  const close = () => setRecap(null);
  const open = (projectId: string, taskId: string) => {
    close();
    navigate(`/p/${projectId}/t/${taskId}`);
  };

  return (
    <AnimatePresence>
      {recap && (
        <motion.section
          aria-label="你不在的时候"
          initial={{ opacity: 0, y: 24 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 16 }}
          transition={{ type: "spring", stiffness: 380, damping: 30 }}
          className="fixed bottom-4 right-4 z-40 w-[340px] max-w-[calc(100vw-32px)] overflow-hidden rounded-card bg-raised shadow-float ring-1 ring-line"
        >
          <header className="flex items-start gap-3 border-b-4 border-double border-line-strong/60 bg-[var(--color-bot-kimi-tint)] px-4 py-3">
            <span className="animate-px-bob">
              <PixelBot member="claude_code" mode="done" size={40} decorative />
            </span>
            <div className="min-w-0 flex-1">
              <div className="font-pixel text-micro tracking-wider text-t3">EXTRA! EXTRA!</div>
              <div className="text-meta font-semibold text-t2">你不在的这 {awayLabel(recap.awayMs)}</div>
              <div className="text-body font-extrabold leading-snug text-t1">{recap.headline}</div>
            </div>
            <button type="button" onClick={close} aria-label="关闭小报" className="grid size-6 place-items-center rounded-row text-t3 hover:bg-raised hover:text-t1">
              ×
            </button>
          </header>
          <div className="flex flex-wrap gap-1.5 px-4 pt-3">
            {ORDER.filter((b) => recap.counts[b] > 0).map((b) => (
              <span key={b} className={cn("rounded-pill px-2 py-0.5 text-micro font-extrabold", BUCKET[b].chip)}>
                {BUCKET[b].label} {recap.counts[b]}
              </span>
            ))}
          </div>
          <ul className="flex flex-col gap-0.5 px-2 py-2">
            {recap.items.map((item) => (
              <li key={item.taskId}>
                <button
                  type="button"
                  onClick={() => open(item.projectId, item.taskId)}
                  className="flex w-full items-center gap-2 rounded-row px-2 py-1.5 text-left hover:bg-app"
                >
                  <span aria-hidden className={cn("size-2 shrink-0 rounded-circle", BUCKET[item.bucket].dot)} />
                  <span className="min-w-0 flex-1 truncate text-meta font-bold text-t1">{item.title}</span>
                  <span className="shrink-0 text-micro font-semibold text-t3">
                    {item.projectName} · {taskCode(item.seq)}
                  </span>
                </button>
              </li>
            ))}
          </ul>
          <footer className="flex justify-end border-t border-line px-3 py-2">
            <button type="button" onClick={close} className="rounded-control bg-action px-3 py-1 text-meta font-bold text-white shadow-press-action hover:brightness-110">
              知道了
            </button>
          </footer>
        </motion.section>
      )}
    </AnimatePresence>
  );
}
