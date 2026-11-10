/**
 * 小屋镜头：中心点（原尺寸像素坐标）+ 缩放（每个原尺寸像素占几个 CSS 像素）。
 * 纯函数，组件只负责把手势换算成这里的调用。
 */
export interface Camera {
  x: number;
  y: number;
  z: number;
}

export interface Viewport {
  w: number;
  h: number;
}

export interface Scene {
  w: number;
  h: number;
}

/** 刚好装下整个房间的缩放。 */
export function containZoom(v: Viewport, s: Scene): number {
  return Math.min(v.w / s.w, v.h / s.h);
}

/** 铺满画面（可能裁掉房间四角留白）的缩放。 */
export function coverZoom(v: Viewport, s: Scene): number {
  return Math.max(v.w / s.w, v.h / s.h);
}

/** 缩放范围：最小刚好铺满（场景四周已经画了花园，不再露出场景外的空白），最大约 3 倍。 */
export function zoomLimits(v: Viewport, s: Scene): { min: number; max: number } {
  const min = coverZoom(v, s);
  return { min, max: min * 3.5 };
}

/**
 * 默认镜头：铺满画面。可以指定对准的点和在「铺满」基础上再放大多少，
 * 例如让屋子占满画面、花园只露一圈。
 */
export function homeCamera(v: Viewport, s: Scene, opts: { center?: { x: number; y: number }; zoom?: number } = {}): Camera {
  const center = opts.center ?? { x: s.w / 2, y: s.h / 2 + s.h * 0.04 };
  return clampCamera({ ...center, z: coverZoom(v, s) * (opts.zoom ?? 1) }, v, s);
}

/** 限制缩放范围，并保证房间不会被拖出画面：镜头中心只能在房间范围内移动，铺满时不露边。 */
export function clampCamera(c: Camera, v: Viewport, s: Scene): Camera {
  const { min, max } = zoomLimits(v, s);
  const z = Math.min(max, Math.max(min, c.z));
  const halfW = v.w / 2 / z, halfH = v.h / 2 / z;
  const clampAxis = (center: number, half: number, size: number) =>
    half * 2 >= size ? size / 2 : Math.min(size - half, Math.max(half, center));
  return { x: clampAxis(c.x, halfW, s.w), y: clampAxis(c.y, halfH, s.h), z };
}

/** 以屏幕上某一点为中心缩放（鼠标 / 双指位置不动）。 */
export function zoomAt(c: Camera, factor: number, screen: { x: number; y: number }, v: Viewport, s: Scene): Camera {
  const wx = c.x + (screen.x - v.w / 2) / c.z;
  const wy = c.y + (screen.y - v.h / 2) / c.z;
  const { min, max } = zoomLimits(v, s);
  const z = Math.min(max, Math.max(min, c.z * factor));
  return clampCamera({ x: wx - (screen.x - v.w / 2) / z, y: wy - (screen.y - v.h / 2) / z, z }, v, s);
}

/** 拖动平移：屏幕上移动 dx/dy 个 CSS 像素。 */
export function panBy(c: Camera, dx: number, dy: number, v: Viewport, s: Scene): Camera {
  return clampCamera({ x: c.x - dx / c.z, y: c.y - dy / c.z, z: c.z }, v, s);
}

/** 原尺寸像素坐标 → 画面上的 CSS 像素坐标。 */
export function toScreen(c: Camera, v: Viewport, x: number, y: number): { x: number; y: number } {
  return { x: (x - c.x) * c.z + v.w / 2, y: (y - c.y) * c.z + v.h / 2 };
}

/** 镜头平滑跟随目标；k 越大越快。 */
export function approach(cur: Camera, target: Camera, dt: number, k = 12): Camera {
  const t = 1 - Math.exp(-dt * k);
  const done = Math.abs(cur.z - target.z) < 1e-3 && Math.abs(cur.x - target.x) < 0.05 && Math.abs(cur.y - target.y) < 0.05;
  if (done) return target;
  return { x: cur.x + (target.x - cur.x) * t, y: cur.y + (target.y - cur.y) * t, z: cur.z + (target.z - cur.z) * t };
}

/** 屏幕坐标（画布 CSS 像素）→ 场景原尺寸像素，toScreen 的反函数。 */
export function toScene(c: Camera, v: Viewport, sx: number, sy: number): { x: number; y: number } {
  return { x: (sx - v.w / 2) / c.z + c.x, y: (sy - v.h / 2) / c.z + c.y };
}
