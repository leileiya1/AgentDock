/** 画面里一个机器人的命中框（场景像素）和它的前后顺序。 */
export interface RobotBox {
  id: string;
  /** 越大越靠前。 */
  depth: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

/** 场景坐标 → 点中的机器人（重叠时取最靠前的）；框外留 2px 余量，像素小人好点一点。 */
export function pickRobot(boxes: readonly RobotBox[], x: number, y: number, pad = 2): string | null {
  let best: RobotBox | null = null;
  for (const b of boxes) {
    if (x < b.x - pad || x > b.x + b.w + pad || y < b.y - pad || y > b.y + b.h + pad) continue;
    if (!best || b.depth > best.depth) best = b;
  }
  return best?.id ?? null;
}
