import { expect, test } from "bun:test";
import { lighten, paintPixels, parseHex } from "./paint";
import { MANIFEST } from "./atlas";
import { TOWN } from "./townAtlas";

test("换色按 所选颜色 × 像素 ÷ 灰度 重算，保留明暗，透明像素不动", () => {
  const px = new Uint8ClampedArray([128, 128, 128, 255, 64, 64, 64, 255, 200, 200, 200, 255, 9, 9, 9, 0]);
  paintPixels(px, [200, 100, 50], 128);
  expect([...px.slice(0, 4)]).toEqual([200, 100, 50, 255]); // 原色
  expect([...px.slice(4, 8)]).toEqual([100, 50, 25, 255]); // 暗面
  expect([...px.slice(8, 12)]).toEqual([255, 156, 78, 255]); // 亮面，截到 255
  expect([...px.slice(12)]).toEqual([9, 9, 9, 0]);
});

test("颜色解析和面板浅色", () => {
  expect(parseHex("#C8553D")).toEqual([200, 85, 61]);
  expect(parseHex("red")).toBeNull();
  expect(lighten([0, 0, 0], 0.5)).toEqual([128, 128, 128]);
});

test("贴图清单里有换色层、配件层和院子摆件", () => {
  for (const theme of ["day", "night"] as const) {
    const custom = MANIFEST[theme].custom;
    for (const ant of Object.values(custom.antenna)) expect(custom.paint[ant]?.["stand-idle"]?.[0]?.base).toBeTruthy();
    for (const acc of ["cap", "headphones", "glasses", "bowtie", "antenna", "scarf"]) expect(custom.acc[acc]?.["stand-idle"]?.length).toBe(2);
    for (const item of ["fountain", "flag", "statue", "garden", "bench", "pet"]) expect(TOWN[theme].yard[item]?.length).toBe(2);
    expect(TOWN[theme].paint.personal[0].lit).toBeTruthy();
  }
});
