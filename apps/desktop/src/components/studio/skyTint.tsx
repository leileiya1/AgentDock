import { useEffect, useState } from "react";
import type { SkyPhase } from "@/lib/studio/sky";

/** 晨昏色：只叠在已经画了东西的像素上（source-atop），不染背景。 */
export function drawTint(ctx: CanvasRenderingContext2D, dpr: number, view: { w: number; h: number }, tint: SkyPhase["tint"]): void {
  if (!tint || tint.alpha <= 0) return;
  const color = getComputedStyle(document.documentElement).getPropertyValue(tint.token).trim();
  if (!color) return;
  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.globalCompositeOperation = "source-atop";
  ctx.globalAlpha = tint.alpha;
  ctx.fillStyle = color;
  ctx.fillRect(0, 0, view.w, view.h);
  ctx.restore();
}

/** 「傍晚 18:20」小牌子，跟着表走。 */
export function SkyChip({ label }: { label: string }) {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 30_000);
    return () => window.clearInterval(id);
  }, []);
  const time = now.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });
  return (
    <span className="rounded-control border border-line-strong bg-raised/90 px-2 py-1 text-micro font-bold text-t2" title="自动模式按本机时间切换昼夜">
      {label} · <span className="font-mono">{time}</span>
    </span>
  );
}
