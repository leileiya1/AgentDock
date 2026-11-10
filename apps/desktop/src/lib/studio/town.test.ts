import { describe, expect, test } from "bun:test";
import type { ProjectSummary } from "@/generated/bindings";
import { facilityCount, layoutCity, spiralOrder } from "./cityLayout";
import { projectLook, residents } from "./townResidents";
import { townAgents, withMembers } from "./projectSummary";

const ids = (n: number) => Array.from({ length: n }, (_, i) => `p${i}`);

describe("小镇布局", () => {
  test("市政厅在正中，每个项目都有且只有一个街区，地图没有空格", () => {
    for (const n of [0, 1, 3, 7, 12, 30]) {
      const city = layoutCity(ids(n));
      expect(city.cells.length).toBe(city.cols * city.rows);
      const hall = city.cells.find((c) => c.type === "facility" && c.facility === "hall")!;
      expect(Math.abs(hall.col - (city.cols - 1) / 2)).toBeLessThanOrEqual(0.5);
      expect(Math.abs(hall.row - (city.rows - 1) / 2)).toBeLessThanOrEqual(0.5);
      const projects = city.cells.filter((c) => c.type === "project").map((c) => (c.type === "project" ? c.projectId : ""));
      expect(projects.sort()).toEqual(ids(n).sort());
      const keys = new Set(city.cells.map((c) => `${c.col},${c.row}`));
      expect(keys.size).toBe(city.cells.length);
    }
  });

  test("项目越多，公共设施越多，至少有公园和游乐园", () => {
    expect(facilityCount(1)).toBe(2);
    expect(facilityCount(6)).toBe(4);
    expect(facilityCount(40)).toBe(6);
    const kinds = layoutCity(ids(1)).cells.filter((c) => c.type === "facility").map((c) => (c.type === "facility" ? c.facility : ""));
    expect(kinds).toEqual(["hall", "park", "amusement"]);
  });

  test("同样的项目顺序得到同样的布局；新加项目不会让旧项目搬家到别处太远", () => {
    expect(layoutCity(ids(5))).toEqual(layoutCity(ids(5)));
    expect(spiralOrder(3, 3)[0]).toEqual({ col: 1, row: 1 });
  });
});

const summary = (over: Partial<ProjectSummary>): ProjectSummary => ({
  projectId: "p",
  taskCount: 3,
  running: 0,
  waitingForYou: 0,
  done: 3,
  failing: 0,
  agents: [],
  activeAgents: [],
  lastActivityAt: new Date().toISOString(),
  assignedAgents: [],
  agentsSource: "derived",
  ...over,
});

describe("房子外观和院子里的机器人", () => {
  test("外观只看汇总数据：在跑冒烟、等你竖旗、失败下雨、没任务是工地", () => {
    const now = Date.now();
    expect(projectLook(summary({ running: 1 }), now)).toMatchObject({ lit: true, smoke: true, flag: false });
    expect(projectLook(summary({ waitingForYou: 2 }), now)).toMatchObject({ flag: true, smoke: false });
    expect(projectLook(summary({ failing: 1 }), now).cloud).toBe(true);
    expect(projectLook(summary({ taskCount: 0 }), now).construction).toBe(true);
    expect(projectLook(summary({ lastActivityAt: "2020-01-01T00:00:00Z" }), now).leaves).toBe(true);
  });

  test("机器人数量 = 这个项目分配过的 Agent 数，不是固定人数", () => {
    const yard = [[20, 60], [30, 62], [40, 64], [60, 60], [70, 62]] as const;
    const door = { x: 48, y: 54 }, mailbox = { x: 60, y: 62 };
    for (const agents of [["claude_code"], ["claude_code", "codex", "qoder_cli"], []]) {
      expect(residents("p", summary({ agents }), door, mailbox, yard, 3).length).toBe(agents.length);
    }
  });

  test("在干活的站门口，等你时第一位举手，其余在院子里溜达", () => {
    const yard = [[20, 60], [30, 62], [40, 64], [60, 60], [70, 62]] as const;
    const rs = residents("p", summary({ agents: ["claude_code", "codex", "qoder_cli"], activeAgents: ["claude_code"], waitingForYou: 1 }), { x: 48, y: 54 }, { x: 60, y: 62 }, yard, 100, true);
    expect(rs.map((r) => r.pose)).toEqual(["stand-work", "stand-hand", "stand-rest"]);
  });
});

describe("项目分配的 Agent", () => {
  const yard = [[20, 60], [30, 62], [40, 64], [60, 60], [70, 62]] as const;
  test("分配了成员就按成员人数和顺序画，不再看最近任务", () => {
    const s = withMembers(summary({ agents: ["claude_code", "codex"] }), ["gemini_cli", "kimi_cli", "codex"]);
    expect(s.agentsSource).toBe("assigned");
    expect(townAgents(s)).toEqual(["gemini_cli", "kimi_cli", "codex"]);
    expect(residents("p", s, { x: 48, y: 54 }, { x: 60, y: 62 }, yard, 3).map((r) => r.agent)).toEqual(["gemini_cli", "kimi_cli", "codex"]);
  });
  test("没分配过（或清空成员）时退回最近任务推导的 agents", () => {
    const s = withMembers(summary({ agents: ["claude_code"] }), []);
    expect(s.agentsSource).toBe("derived");
    expect(townAgents(s)).toEqual(["claude_code"]);
    expect(townAgents(undefined)).toEqual([]);
  });
});

describe("点小镇", () => {
  test("点项目街区的地面就进项目，点公共设施返回 place，点地图外返回 null", async () => {
    const { hitTown, groundAt } = await import("./townHit");
    const { townGeometry, townProject, TOWN } = await import("./townAtlas");
    const city = layoutCity(ids(5));
    const g = townGeometry(city);
    const houses = new Map(ids(5).map((id) => [id, { kind: "personal" as const, tier: 1 as const, look: { lit: true } }]));
    const B = TOWN.block;
    for (const cell of city.cells) {
      // a point near the front corner of the lawn, away from any building
      const [px, py] = townProject(g, cell.col * B + 80, cell.row * B + 80);
      const hit = hitTown(city, g, TOWN.day, houses, px, py);
      if (cell.type === "project") expect(hit).toMatchObject({ type: "project", id: cell.projectId });
      else expect(hit?.type).toBe("place");
    }
    const [x, y] = townProject(g, 10, 20);
    expect(groundAt(g, x, y)).toEqual({ x: 10, y: 20 });
    expect(hitTown(city, g, TOWN.day, houses, -500, -500)).toBeNull();
  });
});
