import { paintPixels, type Rgb } from "@/lib/studio/paint";

type Rect = readonly [number, number, number, number];

const cache = new Map<string, HTMLCanvasElement | null>();
const LIMIT = 600;

/**
 * 从 atlas 里切出一块灰色换色层，按所选颜色重算后缓存成小画布。
 * key 要能区分 atlas 图（白天 / 夜间）和那一块。
 */
export function tinted(img: HTMLImageElement, rect: Rect, color: Rgb, grey: number, key: string): HTMLCanvasElement | null {
  const id = `${key}|${rect.join(",")}|${color.join(",")}`;
  const hit = cache.get(id);
  if (hit !== undefined) return hit;
  let out: HTMLCanvasElement | null = null;
  try {
    const [x, y, w, h] = rect;
    const c = document.createElement("canvas");
    c.width = w;
    c.height = h;
    const g = c.getContext("2d", { willReadFrequently: true });
    if (g) {
      g.drawImage(img, x, y, w, h, 0, 0, w, h);
      const data = g.getImageData(0, 0, w, h);
      paintPixels(data.data, color, grey);
      g.putImageData(data, 0, 0);
      out = c;
    }
  } catch {
    out = null; // a tainted or missing image just keeps the default colour
  }
  if (cache.size >= LIMIT) cache.delete(cache.keys().next().value as string);
  cache.set(id, out);
  return out;
}
