import { describe, expect, test } from "bun:test";
import { JUMP_MS, burst, jumpOffset, showSchedule, step } from "@/components/celebrate/fireworksModel";

const fixed = () => 0.5;

describe("fireworks", () => {
  test("a burst spreads evenly around its origin and cycles colours", () => {
    const ps = burst(100, 50, ["a", "b"], fixed, 8);
    expect(ps).toHaveLength(8);
    expect(ps.map((p) => p.color).slice(0, 3)).toEqual(["a", "b", "a"]);
    const sx = ps.reduce((n, p) => n + p.vx, 0);
    expect(Math.abs(sx)).toBeLessThan(40); // roughly symmetric
  });

  test("particles fall and burn out", () => {
    let ps = burst(0, 0, ["a"], fixed, 4);
    const vy0 = ps[0].vy;
    ps = step(ps, 0.1);
    expect(ps[0].vy).toBeGreaterThan(vy0 * 0.985);
    for (let i = 0; i < 30; i++) ps = step(ps, 0.1);
    expect(ps).toHaveLength(0);
  });

  test("schedule stays in the upper part of the screen", () => {
    for (const s of showSchedule(Math.random, 20)) {
      expect(s.fy).toBeLessThan(0.5);
      expect(s.fx).toBeGreaterThan(0.1);
    }
  });

  test("robot jump bounces then settles", () => {
    expect(jumpOffset(-1)).toBe(0);
    expect(jumpOffset(JUMP_MS / 6)).toBeGreaterThan(0);
    expect(jumpOffset(JUMP_MS)).toBe(0);
  });
});
