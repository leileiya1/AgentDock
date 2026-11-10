/**
 * 换色：贴图脚本把「主人选的颜色」画成灰色（grey），并单独导出受它影响的像素。
 * 运行时按「所选颜色 × 像素值 ÷ grey」逐像素重算，就能得到任意颜色、且保留原来的明暗。
 */

export type Rgb = readonly [number, number, number];

export function parseHex(hex: string | null | undefined): Rgb | null {
  if (!hex || !/^#[0-9a-fA-F]{6}$/.test(hex)) return null;
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** 面板 / 浅色部位：所选颜色往白色靠，接近机器人家族里 base 和 light 的关系。 */
export function lighten(c: Rgb, t = 0.62): Rgb {
  return [0, 1, 2].map((i) => Math.round(c[i] + (255 - c[i]) * t)) as unknown as Rgb;
}

/** 原地把灰色像素换成所选颜色（RGBA，透明像素不动）。 */
export function paintPixels(data: Uint8ClampedArray, color: Rgb, grey: number): void {
  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] === 0) continue;
    data[i] = Math.min(255, Math.round((color[0] * data[i]) / grey));
    data[i + 1] = Math.min(255, Math.round((color[1] * data[i + 1]) / grey));
    data[i + 2] = Math.min(255, Math.round((color[2] * data[i + 2]) / grey));
  }
}
