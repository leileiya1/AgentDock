import { useEffect, useMemo, useRef, useState } from "react";
import { Dialog } from "@/components/Dialog";
import { Button } from "@/components/ui/button";
import { PixelBot, memberShortLabel } from "@/components/pixel/PixelBot";
import { useProjects } from "@/hooks/useProjects";
import { useAllTasks } from "@/hooks/useAllTasks";
import { useAgentProfiles } from "@/hooks/useStudio";
import { weeklyReport } from "@/components/weekly/weeklyModel";
import { CARD, drawReport, svgToImage } from "@/components/weekly/drawReport";
import { toast } from "@/stores/toastStore";

export const OPEN_WEEKLY = "agentflow:weekly";

/** 本周成绩单：画在 canvas 上，一键复制成图片贴到群里。由 OPEN_WEEKLY 事件打开。 */
export function WeeklyReportDialog() {
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const botRef = useRef<HTMLSpanElement>(null);
  const projects = useProjects();
  const profiles = useAgentProfiles();
  const { tasks, key, ready } = useAllTasks(open);

  useEffect(() => {
    const onOpen = () => {
      setCopied(false);
      setOpen(true);
    };
    window.addEventListener(OPEN_WEEKLY, onOpen);
    return () => window.removeEventListener(OPEN_WEEKLY, onOpen);
  }, []);

  const report = useMemo(
    () => weeklyReport(tasks, projects.data ?? [], new Date()),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- key stands in for tasks
    [key, projects.data, open]
  );
  const profile = report.mvp ? profiles.data?.find((p) => p.agent === report.mvp!.agent) : undefined;
  const mvpName = report.mvp ? profile?.nickname || memberShortLabel(report.mvp.agent) : null;

  useEffect(() => {
    if (!open || !ready) return;
    let alive = true;
    // 等对话框和隐藏的小机器人挂上再画
    const id = requestAnimationFrame(async () => {
      const canvas = canvasRef.current;
      const ctx = canvas?.getContext("2d");
      if (!canvas || !ctx) return;
      const svg = botRef.current?.querySelector("svg");
      const bot = svg ? await svgToImage(svg).catch(() => null) : null;
      if (!alive) return;
      await document.fonts?.ready;
      const dpr = 2;
      canvas.width = CARD.w * dpr;
      canvas.height = CARD.h * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      drawReport(ctx, report, { mvpName, bot });
    });
    return () => {
      alive = false;
      cancelAnimationFrame(id);
    };
  }, [open, ready, report, mvpName]);

  const copy = async () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    try {
      const blob = new Promise<Blob>((resolve, reject) => canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("empty"))), "image/png"));
      await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
      setCopied(true);
    } catch {
      toast.error("没能复制图片，可以截图分享");
    }
  };

  return (
    <Dialog
      open={open}
      onClose={() => setOpen(false)}
      title="本周成绩单"
      subtitle="最近 7 天，所有项目一起算"
      width={600}
      footer={
        <>
          <Button variant="outline" onClick={() => setOpen(false)}>关闭</Button>
          <Button onClick={copy} disabled={!ready}>{copied ? "已复制 ✓" : "复制图片"}</Button>
        </>
      }
    >
      <canvas ref={canvasRef} aria-label={`本周评级 ${report.grade}，合并 ${report.merged} 个任务`} role="img" className="block w-full rounded-control" style={{ aspectRatio: `${CARD.w} / ${CARD.h}`, imageRendering: "pixelated" }} />
      {report.mvp && (
        <span ref={botRef} aria-hidden className="pointer-events-none fixed -left-[200px] top-0">
          <PixelBot member={report.mvp.agent} mode="done" size={56} color={profile?.color ?? undefined} decorative />
        </span>
      )}
    </Dialog>
  );
}
