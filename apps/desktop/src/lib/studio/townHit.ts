import type { CityCell, CityLayout } from "./cityLayout";
import type { ProjectKind, Tier } from "./buildings";
import { TOWN, blockOrigin, buildingSprite, type TownGeometry, type TownTheme } from "./townAtlas";

/** 命中判断需要的房子信息。 */
export interface HitHouse {
  kind: ProjectKind;
  tier: Tier;
  look: { lit: boolean };
}

export type TownHit = { type: "project"; id: string; cell: CityCell } | { type: "place"; cell: CityCell } | null;

/** 画布像素 → 地面坐标（城市格，block = 96）。 */
export function groundAt(g: TownGeometry, px: number, py: number): { x: number; y: number } {
  const a = (px - g.ox) / 2; // x - y
  const b = py - g.oy; // x + y
  return { x: (a + b) / 2, y: (b - a) / 2 };
}

/**
 * 点到了哪里：先看房子贴图（从近到远，高楼会挡住身后的街区），再看脚下是哪个街区。
 * 项目街区 → 进项目；公共设施 / 空地 → place（镜头飞过去）。
 */
export function hitTown(city: CityLayout, g: TownGeometry, theme: TownTheme, houses: ReadonlyMap<string, HitHouse>, px: number, py: number): TownHit {
  const projects = city.cells
    .filter((c): c is Extract<CityCell, { type: "project" }> => c.type === "project")
    .sort((a, b) => b.col + b.row - (a.col + a.row));
  for (const cell of projects) {
    const h = houses.get(cell.projectId);
    if (!h) continue;
    const s = buildingSprite(theme, h.kind, h.tier, h.look.lit);
    const [ox, oy] = blockOrigin(g, cell);
    const x0 = ox + s.dx, y0 = oy + s.dy;
    // 只算贴图下半部分偏中间的区域，避免宽大的地基和烟雾范围误点
    if (px >= x0 + s.w * 0.12 && px <= x0 + s.w * 0.88 && py >= y0 && py <= y0 + s.h * 0.9) return { type: "project", id: cell.projectId, cell };
  }
  const ground = groundAt(g, px, py);
  const col = Math.floor(ground.x / TOWN.block), row = Math.floor(ground.y / TOWN.block);
  const cell = city.cells.find((c) => c.col === col && c.row === row);
  if (!cell) return null;
  return cell.type === "project" ? { type: "project", id: cell.projectId, cell } : { type: "place", cell };
}
