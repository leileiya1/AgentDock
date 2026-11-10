import { describe, expect, test } from "bun:test";
import { depthOf, poseFor, stepActor, syncActors, type Actor } from "./motion";
import type { Placement } from "./assign";
import { spotById } from "./layout";

const place = (id: string, spot: string | null, activity: Placement["activity"] = "idle"): Placement => ({ id, spot, activity, taskCode: null, taskId: null, overflow: spot === null });

describe("机器人移动", () => {
  test("新成员直接出现在点位上，不满屋乱跑", () => {
    const actors = syncActors(new Map(), [place("grok", "sofa-1")], true);
    expect(actors.get("grok")).toMatchObject({ spot: "sofa-1", pos: spotById("sofa-1")!.at, path: [] });
  });

  test("换点位时沿走道走过去，走完到达目的地", () => {
    let actors = syncActors(new Map(), [place("grok", "sofa-1")], true);
    actors = syncActors(actors, [place("grok", "desk-3", "develop")], true);
    let a = actors.get("grok")!;
    expect(a.path.length).toBeGreaterThan(1);
    expect(poseFor(a, "develop")).toMatch(/^walk-/);
    for (let i = 0; i < 400 && a.path.length; i++) a = stepActor(a, 1 / 30);
    expect(a.path).toEqual([]);
    expect(a.pos).toEqual(spotById("desk-3")!.at);
    expect(poseFor(a, "develop")).toBe("sit-back");
    expect(depthOf(a)).toBe(spotById("desk-3")!.depth);
  });

  test("减少动态效果时直接瞬移", () => {
    let actors = syncActors(new Map(), [place("grok", "sofa-1")], false);
    actors = syncActors(actors, [place("grok", "desk-3", "develop")], false);
    expect(actors.get("grok")).toMatchObject({ pos: spotById("desk-3")!.at, path: [] });
  });

  test("走到一半换目的地：先走完当前这段再接新路线", () => {
    let actors = syncActors(new Map(), [place("grok", "sofa-1")], true);
    actors = syncActors(actors, [place("grok", "desk-3", "develop")], true);
    let a: Actor = actors.get("grok")!;
    a = stepActor(a, 0.5);
    const remaining = a.path.length;
    actors = syncActors(new Map([["grok", a]]), [place("grok", "tea")], true);
    const b = actors.get("grok")!;
    expect(b.spot).toBe("tea");
    expect(b.path.length).toBeGreaterThan(remaining);
    expect(b.path.at(-1)).toEqual(spotById("tea")!.at);
  });

  test("没有点位（工位满了）的成员不画在场景里", () => {
    expect(syncActors(new Map(), [place("x", null, "develop")], true).size).toBe(0);
  });

  test("走路朝向：朝镜头是正面，背对是背面", () => {
    const toward: Actor = { id: "a", spot: "tea", pos: { x: 10, y: 10 }, path: [{ x: 10, y: 30 }], facing: "back" };
    expect(stepActor(toward, 0.1).facing).toBe("front");
    const away: Actor = { id: "a", spot: "tea", pos: { x: 10, y: 30 }, path: [{ x: 10, y: 10 }], facing: "front" };
    expect(stepActor(away, 0.1).facing).toBe("back");
  });

  test("站着的姿势跟活动走：等你举手、茶水吧端杯子、空闲休息", () => {
    const at = (spot: string): Actor => ({ id: "a", spot, pos: spotById(spot)!.at, path: [], facing: "front" });
    expect(poseFor(at("queue-1"), "waiting")).toBe("stand-hand");
    expect(poseFor(at("tea"), "idle")).toBe("stand-cup");
    expect(poseFor(at("arcade"), "idle")).toBe("stand-rest");
    expect(poseFor(at("whiteboard"), "plan")).toBe("stand-work");
    expect(poseFor(at("beanbag"), "idle")).toBe("sleep");
  });
});
