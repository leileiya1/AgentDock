/**
 * 小镇布局：把项目和公共设施排进一个方形街区网格。
 *
 * - 市政厅永远在正中；项目和公共设施从中心一圈圈往外排，项目之间穿插公园、游乐园等，
 *   看起来像一座城市而不是一排宿舍。
 * - 项目越多，城市越大、公共设施越多；空出来的街区用森林营地、草坪补齐，地图永远是满的。
 * - 只依赖项目顺序（创建顺序），同样的输入永远得到同样的布局，房子不会乱跳。
 */

export type FacilityKind = "hall" | "park" | "amusement" | "lake" | "market" | "stadium" | "garden";
export type FillerKind = "forest" | "lawn";

export type CityCell =
  | { col: number; row: number; type: "project"; projectId: string }
  | { col: number; row: number; type: "facility"; facility: FacilityKind }
  | { col: number; row: number; type: "filler"; filler: FillerKind };

export interface CityLayout {
  cols: number;
  rows: number;
  cells: CityCell[];
}

/** 按项目数逐步解锁的公共设施，越靠前越早出现。 */
export const FACILITY_ORDER: readonly FacilityKind[] = ["park", "amusement", "lake", "market", "stadium", "garden"];

export const FACILITY_LABEL: Record<FacilityKind | FillerKind, string> = {
  hall: "市政厅广场",
  park: "中央公园",
  amusement: "游乐园",
  lake: "湖心亭",
  market: "集市",
  stadium: "体育场",
  garden: "植物园",
  forest: "营地森林",
  lawn: "草坪",
};

/** 项目数 → 公共设施数：至少两处，之后每多两个项目多一处，最多全开。 */
export function facilityCount(projects: number): number {
  return Math.min(FACILITY_ORDER.length, Math.max(2, Math.ceil(projects / 2) + 1));
}

/** 网格里各格按「离中心多近」排序：先中心，再一圈圈往外，同一圈内按固定方向。 */
export function spiralOrder(cols: number, rows: number): Array<{ col: number; row: number }> {
  const cc = (cols - 1) / 2, cr = (rows - 1) / 2;
  const cells: Array<{ col: number; row: number; ring: number; angle: number }> = [];
  for (let row = 0; row < rows; row++) {
    for (let col = 0; col < cols; col++) {
      const dx = col - cc, dy = row - cr;
      cells.push({ col, row, ring: Math.max(Math.abs(dx), Math.abs(dy)), angle: Math.atan2(dy, dx) });
    }
  }
  cells.sort((a, b) => a.ring - b.ring || a.angle - b.angle || a.row - b.row || a.col - b.col);
  return cells.map(({ col, row }) => ({ col, row }));
}

export function layoutCity(projectIds: readonly string[]): CityLayout {
  const facilities = FACILITY_ORDER.slice(0, facilityCount(projectIds.length));
  const needed = 1 + projectIds.length + facilities.length;
  let cols = Math.max(3, Math.ceil(Math.sqrt(needed)));
  let rows = Math.max(3, Math.ceil(needed / cols));
  if (cols * rows < needed) rows++;
  if (cols < rows) [cols, rows] = [rows, cols];

  const order = spiralOrder(cols, rows);
  const cells: CityCell[] = [];
  const [hall, ...rest] = order;
  cells.push({ ...hall, type: "facility", facility: "hall" });

  let p = 0, f = 0;
  rest.forEach((pos, index) => {
    // 每三格里放一处公共设施，其余给项目；某一类用完就全给另一类
    const wantFacility = index % 3 === 2;
    if (p < projectIds.length && (!wantFacility || f >= facilities.length)) {
      cells.push({ ...pos, type: "project", projectId: projectIds[p++] });
    } else if (f < facilities.length) {
      cells.push({ ...pos, type: "facility", facility: facilities[f++] });
    } else {
      cells.push({ ...pos, type: "filler", filler: (pos.col + pos.row) % 2 === 0 ? "forest" : "lawn" });
    }
  });
  return { cols, rows, cells };
}
