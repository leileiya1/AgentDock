/**
 * 项目小镇的建筑图鉴（预留口子）：项目「类型」决定房子造型，项目规模决定房子大小，
 * 主人可以再改屋顶颜色、门牌样式和院子摆件。
 *
 * - 类型由用户在项目设置里选；没选时按项目名猜一个，并标明是猜的，界面上提示去改。
 * - 新增类型：在 PROJECT_KINDS 加一项，在贴图脚本 scripts/studio-art 里加对应的造型函数并重新导出。
 *   未知类型（例如旧版本保存的、或以后服务端下发的新类型）一律按「民房」画，不会报错。
 * - 只描述外观；外观上的状态（冒烟、小旗、雨云、落叶）仍然只来自后端任务状态。
 */

export const PROJECT_KINDS = ["personal", "team", "experiment", "client", "backend", "data", "docs", "ops"] as const;
export type ProjectKind = (typeof PROJECT_KINDS)[number];

export interface BuildingStyle {
  kind: ProjectKind;
  /** 设置里给用户看的类型名。 */
  label: string;
  /** 这一类的房子叫什么。 */
  building: string;
  /** 一句话说明适合什么项目。 */
  fits: string;
  /** 这种造型上，状态会显示在哪里（给图例和读屏用）。 */
  signals: string;
}

export const BUILDINGS: Record<ProjectKind, BuildingStyle> = {
  personal: { kind: "personal", label: "个人项目", building: "民房", fits: "自己的小项目、练手、副业", signals: "烟囱冒烟 = 在干活；窗户亮几扇 = 几个任务在跑" },
  team: { kind: "team", label: "团队 / 公司项目", building: "写字楼", fits: "多人协作、正式产品", signals: "亮着的楼层 = 在跑的任务；楼顶红灯闪 = 有事等你" },
  experiment: { kind: "experiment", label: "实验 / 原型", building: "车库工坊", fits: "试新技术、做 Demo、黑客松", signals: "卷帘门拉开、火花四溅 = 在干活；门关着 = 歇着" },
  client: { kind: "client", label: "客户交付", building: "商铺", fits: "外包、给客户做的版本", signals: "橱窗亮灯 = 营业中；门口邮箱小旗 = 客户在等你确认" },
  backend: { kind: "backend", label: "后端 / 基础设施", building: "工厂", fits: "服务端、API、构建流水线", signals: "烟囱白烟 = 正常运转；灰烟 = 测试失败" },
  data: { kind: "data", label: "数据 / AI", building: "天文台", fits: "数据管道、模型训练、分析", signals: "圆顶缝打开、望远镜转动 = 在算；夜里头顶有星星" },
  docs: { kind: "docs", label: "文档 / 开源", building: "图书馆", fits: "文档站、开源库、知识库", signals: "门口长椅有人 = 在写；窗户亮灯 = 在审" },
  ops: { kind: "ops", label: "运维 / 监控", building: "灯塔", fits: "部署、告警、值班", signals: "灯塔转光 = 正常巡检；灯灭 = 很久没动" },
};

/** 规模档位：1 小、2 中、3 大。 */
export type Tier = 1 | 2 | 3;

export interface ProjectScale {
  /** 最近 30 天参与过的 Agent 数。 */
  agents: number;
  /** 累计任务数。 */
  tasks: number;
}

/** 规模越大房子越大：民房加一层、写字楼加高。阈值保守，避免小项目一夜变大楼。 */
export function tierFor(scale: ProjectScale): Tier {
  if (scale.agents >= 4 || scale.tasks >= 60) return 3;
  if (scale.agents >= 2 || scale.tasks >= 15) return 2;
  return 1;
}

/** 院子摆件：成就或用户自己选的装饰（预留）。 */
export type YardItem = "fountain" | "flag" | "statue" | "garden" | "bench" | "pet";

/** 用户对项目外观的设置（预留，将来存后端，换设备不丢）。所有字段都可省略。 */
export interface ProjectLook {
  kind?: string;
  /** 屋顶 / 主色，CSS 颜色。 */
  roof?: string;
  /** 门牌样式。 */
  sign?: "wood" | "neon" | "stone";
  yard?: YardItem[];
}

const SIGNS = new Set<ProjectLook["sign"]>(["wood", "neon", "stone"]);
const YARD = new Set<string>(["fountain", "flag", "statue", "garden", "bench", "pet"]);

/** 后端存的外观 → 前端 ProjectLook。后端已校验；这里只再挡一下新版本加的、本版本不认识的值。 */
export function lookFromAppearance(a: { kind: string | null; roof: string | null; sign: string | null; yard: string[] }): ProjectLook {
  return {
    kind: a.kind ?? undefined,
    roof: a.roof ?? undefined,
    sign: SIGNS.has(a.sign as ProjectLook["sign"]) ? (a.sign as ProjectLook["sign"]) : undefined,
    yard: a.yard.filter((y): y is YardItem => YARD.has(y)),
  };
}

const HINTS: Array<[RegExp, ProjectKind]> = [
  [/(^|[-_ ])(docs?|wiki|handbook|book|site)([-_ ]|$)/i, "docs"],
  [/(infra|ops|deploy|devops|k8s|terraform|monitor|alert)/i, "ops"],
  [/(data|etl|pipeline|ml|ai|model|analytics|train)/i, "data"],
  [/(api|server|backend|service|gateway|worker)/i, "backend"],
  [/(demo|poc|lab|playground|experiment|prototype|hack|sandbox)/i, "experiment"],
  [/(client|customer|agency|外包|客户)/i, "client"],
];

/** 用户没选类型时，按项目名猜一个；猜不出就是民房。 */
export function guessKind(projectName: string): ProjectKind {
  for (const [re, kind] of HINTS) if (re.test(projectName)) return kind;
  return "personal";
}

export function isProjectKind(value: unknown): value is ProjectKind {
  return typeof value === "string" && (PROJECT_KINDS as readonly string[]).includes(value);
}

export interface BuildingChoice {
  kind: ProjectKind;
  tier: Tier;
  /** chosen = 用户选的；guessed = 按名字猜的（界面提示「去设置里改」）；fallback = 存的是认不出的类型。 */
  source: "chosen" | "guessed" | "fallback";
  style: BuildingStyle;
}

/** 决定一个项目的房子长什么样。 */
export function buildingFor(projectName: string, look: ProjectLook | undefined, scale: ProjectScale): BuildingChoice {
  const tier = tierFor(scale);
  if (look?.kind !== undefined) {
    if (isProjectKind(look.kind)) return { kind: look.kind, tier, source: "chosen", style: BUILDINGS[look.kind] };
    return { kind: "personal", tier, source: "fallback", style: BUILDINGS.personal };
  }
  const kind = guessKind(projectName);
  return { kind, tier, source: "guessed", style: BUILDINGS[kind] };
}
