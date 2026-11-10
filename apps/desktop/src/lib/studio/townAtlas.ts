import townJson from "@/assets/town/town-manifest.json";
import type { ProjectKind, Tier } from "./buildings";
import type { CityCell, CityLayout } from "./cityLayout";

/** atlas 里的一块：[x, y, w, h]。dx/dy 是相对街区原点 P(0,0,0) 的偏移。 */
export interface TownSprite {
  dx: number;
  dy: number;
  w: number;
  h: number;
  rect: [number, number, number, number];
}

export interface TownTheme {
  tiles: Record<string, TownSprite[]>;
  buildings: Record<ProjectKind, Array<{ lit: TownSprite; dark: TownSprite }>>;
  overlays: Record<"smoke" | "flag" | "cloud" | "leaves" | "construction", TownSprite[]>;
  /** 每种房子、每档规模的烟囱口（相对街区原点的屏幕像素）。 */
  smoke: Record<ProjectKind, Array<Array<[number, number]>>>;
  /** 主人选的屋顶 / 主色：受它影响的像素（灰色画的），按 kind → 档位 → 亮 / 暗。 */
  paint: Record<ProjectKind, Array<{ lit: TownSprite | null; dark: TownSprite | null }>>;
  /** 院子摆件，每样 2 帧，各自固定在院子里的位置。 */
  yard: Record<string, TownSprite[]>;
}

export interface TownManifest {
  block: number;
  tileFrames: number;
  sign: [number, number, number];
  mailbox: [number, number];
  yard: Array<[number, number]>;
  /** 换色层里「所选颜色」画成的灰度值。 */
  paintGrey: number;
  facilities: Record<string, string>;
  day: TownTheme;
  night: TownTheme;
}

export const TOWN = townJson as unknown as TownManifest;

/** 街区高处留白：摩天轮、写字楼会高出地面菱形。 */
export const TOWN_TOP = 180;
const MARGIN = 24;

export interface TownGeometry {
  w: number;
  h: number;
  ox: number;
  oy: number;
}

/** 整座城市的画布尺寸和原点：cols × rows 个 96 格街区，2:1 等距斜俯视。 */
export function townGeometry(city: Pick<CityLayout, "cols" | "rows">): TownGeometry {
  const B = TOWN.block;
  return {
    w: 2 * (city.cols + city.rows) * B + 2 * MARGIN,
    h: (city.cols + city.rows) * B + TOWN_TOP + 2 * MARGIN,
    ox: 2 * city.rows * B + MARGIN,
    oy: TOWN_TOP + MARGIN,
  };
}

/** 城市坐标（地面格）→ 画布像素。 */
export function townProject(g: TownGeometry, x: number, y: number, z = 0): [number, number] {
  return [g.ox + 2 * (x - y), g.oy + (x + y) - z];
}

/** 某个街区原点在画布上的位置。 */
export function blockOrigin(g: TownGeometry, cell: Pick<CityCell, "col" | "row">): [number, number] {
  return townProject(g, cell.col * TOWN.block, cell.row * TOWN.block);
}

/** 街区的地面贴图键。 */
export function tileKey(cell: CityCell): string {
  if (cell.type === "project") return "project";
  if (cell.type === "facility") return cell.facility;
  return cell.filler;
}

export function buildingSprite(theme: TownTheme, kind: ProjectKind, tier: Tier, lit: boolean): TownSprite {
  const set = theme.buildings[kind]?.[tier - 1] ?? theme.buildings.personal[0];
  return lit ? set.lit : set.dark;
}

export function paintSprite(theme: TownTheme, kind: ProjectKind, tier: Tier, lit: boolean): TownSprite | null {
  const set = theme.paint?.[kind]?.[tier - 1];
  return (lit ? set?.lit : set?.dark) ?? null;
}
