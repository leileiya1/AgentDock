import { describe, expect, test } from "bun:test";
import { BUILDINGS, PROJECT_KINDS, buildingFor, guessKind, lookFromAppearance, tierFor } from "./buildings";

describe("项目小镇建筑图鉴", () => {
  test("每个类型都有完整的图鉴条目", () => {
    for (const kind of PROJECT_KINDS) {
      const style = BUILDINGS[kind];
      expect(style.kind).toBe(kind);
      expect(style.label.length).toBeGreaterThan(0);
      expect(style.building.length).toBeGreaterThan(0);
      expect(style.signals.length).toBeGreaterThan(0);
    }
  });

  test("用户选了类型就按选的来", () => {
    expect(buildingFor("acme-web", { kind: "team" }, { agents: 1, tasks: 3 })).toMatchObject({ kind: "team", source: "chosen" });
  });

  test("没选时按名字猜，猜不出是民房", () => {
    expect(guessKind("docs-site")).toBe("docs");
    expect(guessKind("infra-deploy")).toBe("ops");
    expect(guessKind("data-pipeline")).toBe("data");
    expect(guessKind("payment-api")).toBe("backend");
    expect(guessKind("rust-playground")).toBe("experiment");
    expect(guessKind("my-blog")).toBe("personal");
    expect(buildingFor("data-pipeline", undefined, { agents: 1, tasks: 1 })).toMatchObject({ kind: "data", source: "guessed" });
  });

  test("认不出的类型（旧数据或以后新增的）按民房画，不报错", () => {
    expect(buildingFor("x", { kind: "spaceship" }, { agents: 1, tasks: 1 })).toMatchObject({ kind: "personal", source: "fallback" });
  });

  test("规模越大房子越大，小项目不会一下变大楼", () => {
    expect(tierFor({ agents: 1, tasks: 3 })).toBe(1);
    expect(tierFor({ agents: 2, tasks: 3 })).toBe(2);
    expect(tierFor({ agents: 1, tasks: 20 })).toBe(2);
    expect(tierFor({ agents: 4, tasks: 5 })).toBe(3);
    expect(tierFor({ agents: 1, tasks: 80 })).toBe(3);
  });
});

test("后端保存的外观转成 ProjectLook：认识的类型直接用，新版本的门牌和摆件先忽略", () => {
  const look = lookFromAppearance({ kind: "ops", roof: "#336699", sign: "hologram", yard: ["pet", "rocket"] });
  expect(look).toEqual({ kind: "ops", roof: "#336699", sign: undefined, yard: ["pet"] });
  expect(buildingFor("my-blog", look, { agents: 1, tasks: 1 })).toMatchObject({ kind: "ops", source: "chosen" });
  expect(lookFromAppearance({ kind: null, roof: null, sign: null, yard: [] }).kind).toBeUndefined();
});
