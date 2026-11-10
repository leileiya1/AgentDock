/** 像素烟花的纯逻辑：粒子生成、物理步进、机器人蹦跳曲线。画布只负责画。 */

export interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  /** 剩余寿命（秒）。 */
  life: number;
  color: string;
}

export const GRAVITY = 220; // px/s²
export const DRAG = 0.985;

/** 一朵烟花：count 个粒子从 (x, y) 均匀炸开，速度带一点随机。 */
export function burst(x: number, y: number, colors: readonly string[], rand: () => number = Math.random, count = 36): Particle[] {
  const out: Particle[] = [];
  for (let i = 0; i < count; i++) {
    const a = (i / count) * Math.PI * 2 + rand() * 0.2;
    const speed = 90 + rand() * 90;
    out.push({ x, y, vx: Math.cos(a) * speed, vy: Math.sin(a) * speed, life: 0.9 + rand() * 0.6, color: colors[i % colors.length] });
  }
  return out;
}

/** 前进 dt 秒，去掉烧完的。 */
export function step(particles: readonly Particle[], dt: number): Particle[] {
  const out: Particle[] = [];
  for (const p of particles) {
    const life = p.life - dt;
    if (life <= 0) continue;
    out.push({ x: p.x + p.vx * dt, y: p.y + p.vy * dt, vx: p.vx * DRAG, vy: p.vy * DRAG + GRAVITY * dt, life, color: p.color });
  }
  return out;
}

/** 这场庆祝要在什么时刻、什么位置放几朵（相对画面宽高的比例）。 */
export function showSchedule(rand: () => number = Math.random, shots = 5): Array<{ at: number; fx: number; fy: number }> {
  return Array.from({ length: shots }, (_, i) => ({ at: i * 0.32 + rand() * 0.12, fx: 0.15 + rand() * 0.7, fy: 0.12 + rand() * 0.3 }));
}

export const JUMP_MS = 1600;

/** 合并后机器人原地蹦三下：返回向上的位移（场景像素），结束后为 0。 */
export function jumpOffset(sinceMs: number, height = 6): number {
  if (sinceMs < 0 || sinceMs >= JUMP_MS) return 0;
  const t = sinceMs / JUMP_MS;
  return Math.round(Math.abs(Math.sin(t * Math.PI * 3)) * height * (1 - t * 0.5));
}
