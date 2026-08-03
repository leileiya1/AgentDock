import { Clock, Pause, TriangleAlert } from "lucide-react";
import { useSettings, useUpdateSettings } from "@/hooks/useSettings";
import { useOnboarding } from "@/hooks/useEnv";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { Button } from "@/components/ui/button";

function withinWindow(start: string | null | undefined, end: string | null | undefined, now = new Date()): boolean {
  if (!start || !end) return true;
  const minutes = now.getHours() * 60 + now.getMinutes();
  const parse = (value: string) => {
    const [h = "0", m = "0"] = value.split(":");
    return Number(h) * 60 + Number(m);
  };
  const from = parse(start);
  const to = parse(end);
  // 跨午夜的运行窗口（例如 22:00–06:00）。
  return from <= to ? minutes >= from && minutes <= to : minutes >= from || minutes <= to;
}

/**
 * 全局调度状态横幅 (05 §6.7). 覆盖三件用户必须知道的事：
 *   • 全局暂停时给出明确横幅和恢复按钮；
 *   • 运行窗口外说明下次允许启动时间；
 *   • 后台服务不可用时区分「UI 断连」和「Agent 已停止」，避免制造恐慌 (05 §6.7)。
 */
export function SchedulerBanner() {
  const settings = useSettings();
  const onboarding = useOnboarding();
  const update = useUpdateSettings();

  const paused = settings.data?.schedulerPaused === true;
  const start = settings.data?.runWindowStart;
  const end = settings.data?.runWindowEnd;
  const outsideWindow = !paused && !withinWindow(start, end);
  const daemonDown = onboarding.data ? !onboarding.data.daemonRunning : false;

  if (!paused && !outsideWindow && !daemonDown) return null;

  const resume = async () => {
    try {
      await update.mutateAsync({ schedulerPaused: false });
      toast.info("已恢复调度，排队中的任务会陆续开始");
    } catch (error) {
      toast.error(errorLine(error));
    }
  };

  if (daemonDown) {
    return (
      <Banner tone="attention" icon={<TriangleAlert className="size-4" aria-hidden />}>
        <span>
          <strong className="font-medium">界面暂时连不上后台服务</strong>
          ——已经在运行的 Agent 不会因此停止，重新连上后日志会继续同步。
        </span>
      </Banner>
    );
  }

  if (paused) {
    return (
      <Banner
        tone="attention"
        icon={<Pause className="size-4" aria-hidden />}
        action={
          <Button size="sm" variant="human" disabled={update.isPending} onClick={resume}>
            恢复调度
          </Button>
        }
      >
        <span>
          <strong className="font-medium">调度已全局暂停</strong>
          ——不会开始新的运行，正在运行的 Agent 会跑完当前这一步。
        </span>
      </Banner>
    );
  }

  return (
    <Banner tone="idle" icon={<Clock className="size-4" aria-hidden />}>
      <span>
        当前不在运行窗口内，下次允许启动时间为 <strong className="font-medium">{start}</strong>。
        排队中的任务会等到那时自动开始。
      </span>
    </Banner>
  );
}

function Banner({
  tone,
  icon,
  action,
  children,
}: {
  tone: "attention" | "idle";
  icon: React.ReactNode;
  action?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div
      role="status"
      className={`flex shrink-0 items-center gap-2 border-b px-4 py-2 text-body ${
        tone === "attention" ? "border-status-human/50 bg-status-human-bg text-t1" : "border-line bg-panel/70 text-t2"
      }`}
    >
      <span className={tone === "attention" ? "text-status-human" : "text-t3"}>{icon}</span>
      <span className="min-w-0 flex-1">{children}</span>
      {action}
    </div>
  );
}
