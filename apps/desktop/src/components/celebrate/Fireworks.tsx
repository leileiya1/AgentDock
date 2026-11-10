import { useEffect, useRef } from "react";
import { AnimatePresence, motion } from "motion/react";
import { useCelebrateStore } from "@/stores/celebrateStore";
import { burst, showSchedule, step, type Particle } from "@/components/celebrate/fireworksModel";

const SHOW_MS = 2600;
const PX = 4; // 粒子是 4×4 的像素方块
const COLOR_TOKENS = ["--color-bot-kimi", "--color-bot-claude", "--color-bot-codex", "--color-bot-gemini", "--color-bot-qoder", "--color-status-human"];

function reducedMotion(): boolean {
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

/** 合并庆祝：全屏像素烟花 + 顶部横幅，一场约 2.6 秒，排队依次放。减少动态效果时只出横幅。 */
export function Fireworks() {
  const current = useCelebrateStore((s) => s.queue[0]);
  const done = useCelebrateStore((s) => s.done);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (!current) return;
    const timer = window.setTimeout(() => done(current.id), SHOW_MS);
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx || reducedMotion()) return () => window.clearTimeout(timer);

    const css = getComputedStyle(document.documentElement);
    const colors = COLOR_TOKENS.map((t) => css.getPropertyValue(t).trim()).filter(Boolean);
    const w = window.innerWidth, h = window.innerHeight;
    const dpr = Math.max(1, window.devicePixelRatio || 1);
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.imageSmoothingEnabled = false;

    const shots = showSchedule();
    let fired = 0;
    let particles: Particle[] = [];
    let elapsed = 0;
    let last = performance.now();
    let raf = 0;
    const loop = (now: number) => {
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;
      elapsed += dt;
      while (fired < shots.length && shots[fired].at <= elapsed) {
        const s = shots[fired++];
        particles = particles.concat(burst(s.fx * w, s.fy * h, colors));
      }
      particles = step(particles, dt);
      ctx.clearRect(0, 0, w, h);
      for (const p of particles) {
        ctx.globalAlpha = Math.min(1, p.life * 1.5);
        ctx.fillStyle = p.color;
        ctx.fillRect(Math.round(p.x / PX) * PX, Math.round(p.y / PX) * PX, PX, PX);
      }
      ctx.globalAlpha = 1;
      if (fired < shots.length || particles.length) raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => {
      window.clearTimeout(timer);
      cancelAnimationFrame(raf);
      ctx.clearRect(0, 0, w, h);
    };
  }, [current, done]);

  return (
    <>
      <canvas ref={canvasRef} aria-hidden className="pointer-events-none fixed inset-0 z-50 size-full" style={{ imageRendering: "pixelated" }} />
      <AnimatePresence>
        {current && (
          <motion.div
            key={current.id}
            role="status"
            initial={{ opacity: 0, y: -24, scale: 0.9 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -12 }}
            transition={{ type: "spring", stiffness: 420, damping: 22 }}
            className="pointer-events-none fixed inset-x-0 top-6 z-50 flex justify-center"
          >
            <div className="flex items-center gap-2 rounded-card bg-[var(--color-bot-kimi-light)] px-4 py-2 shadow-float ring-2 ring-[var(--color-bot-kimi)]">
              <span className="animate-px-bob font-pixel text-section text-caution">★</span>
              <span className="text-body font-extrabold text-t1">{current.text}</span>
              <span className="animate-px-bob font-pixel text-section text-caution">★</span>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </>
  );
}
