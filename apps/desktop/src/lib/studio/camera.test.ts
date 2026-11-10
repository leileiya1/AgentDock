import { describe, expect, test } from "bun:test";
import { approach, clampCamera, containZoom, coverZoom, homeCamera, panBy, toScreen, zoomAt, zoomLimits } from "./camera";

const S = { w: 488, h: 316 };
const V = { w: 1600, h: 720 };

describe("小屋镜头", () => {
  test("默认镜头铺满画面，不留左右空白", () => {
    const c = homeCamera(V, S);
    expect(c.z).toBeCloseTo(coverZoom(V, S));
    const left = toScreen(c, V, 0, c.y).x, right = toScreen(c, V, S.w, c.y).x;
    expect(left).toBeLessThanOrEqual(0.5);
    expect(right).toBeGreaterThanOrEqual(V.w - 0.5);
  });

  test("缩放以鼠标位置为中心：那一点在屏幕上不动", () => {
    const c = homeCamera(V, S);
    const p = { x: 400, y: 300 };
    const worldX = c.x + (p.x - V.w / 2) / c.z;
    const z2 = zoomAt(c, 1.5, p, V, S);
    expect(toScreen(z2, V, worldX, c.y + (p.y - V.h / 2) / c.z).x).toBeCloseTo(p.x, 0);
    expect(z2.z).toBeCloseTo(c.z * 1.5);
  });

  test("缩放有上下限", () => {
    const { min, max } = zoomLimits(V, S);
    const c = homeCamera(V, S);
    expect(zoomAt(c, 100, { x: 0, y: 0 }, V, S).z).toBeCloseTo(max);
    expect(zoomAt(c, 0.001, { x: 0, y: 0 }, V, S).z).toBeCloseTo(min);
    expect(min).toBeCloseTo(coverZoom(V, S));
    expect(min).toBeGreaterThanOrEqual(containZoom(V, S));
  });

  test("拖动和缩小都不会露出场景外的空白", () => {
    const c = homeCamera(V, S);
    const far = panBy(c, 100000, 100000, V, S);
    expect(far.x).toBeGreaterThanOrEqual(V.w / 2 / far.z - 1e-6);
    expect(far.y).toBeGreaterThanOrEqual(V.h / 2 / far.z - 1e-6);
    const small = clampCamera({ x: 0, y: 0, z: 0.01 }, V, S);
    expect(small.z).toBeCloseTo(coverZoom(V, S));
    expect(toScreen(small, V, 0, 0).x).toBeLessThanOrEqual(0.5);
    expect(toScreen(small, V, 0, 0).y).toBeLessThanOrEqual(0.5);
  });

  test("镜头平滑靠近目标，最后精确到达", () => {
    let c = { x: 0, y: 0, z: 1 };
    const target = { x: 100, y: 50, z: 3 };
    for (let i = 0; i < 200; i++) c = approach(c, target, 1 / 60);
    expect(c).toEqual(target);
  });
});
