import { expect, test } from "bun:test";
import { skyFor, skyPhase } from "@/lib/studio/sky";

const at = (h: number, m = 0) => new Date(2026, 6, 30, h, m);

test("phases follow the clock", () => {
  expect(skyPhase(at(12)).name).toBe("day");
  expect(skyPhase(at(12)).tint).toBeNull();
  expect(skyPhase(at(6, 30))).toMatchObject({ name: "predawn", theme: "night", sleepy: true });
  expect(skyPhase(at(7, 30))).toMatchObject({ name: "dawn", theme: "day" });
  expect(skyPhase(at(18))).toMatchObject({ name: "dusk", theme: "day", label: "傍晚" });
  expect(skyPhase(at(21)).label).toBe("夜晚");
  expect(skyPhase(at(2)).label).toBe("深夜");
});

test("dusk deepens, dawn fades", () => {
  expect(skyPhase(at(18, 50)).tint!.alpha).toBeGreaterThan(skyPhase(at(17, 10)).tint!.alpha);
  expect(skyPhase(at(7, 50)).tint!.alpha).toBeLessThan(skyPhase(at(7, 5)).tint!.alpha);
});

test("manual choice overrides the clock", () => {
  expect(skyFor("day", at(22))).toMatchObject({ theme: "day", tint: null, sleepy: false });
  expect(skyFor("night", at(12)).theme).toBe("night");
  expect(skyFor("auto", at(22)).theme).toBe("night");
});
