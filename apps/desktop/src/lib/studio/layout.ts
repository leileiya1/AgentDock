/**
 * 像素小屋的布局数据（与贴图生成脚本 scripts/studio-art/room.py 同一坐标系）。
 *
 * 坐标单位是地面格：x 沿右墙从后角往前，y 沿左墙从后角往前。画面是 2:1 等距
 * 斜俯视（屏幕 x = OX + 2·(x−y)，屏幕 y = OY + x + y − z），由渲染层负责投影。
 * 房间四周是花园（只是背景），机器人只在屋里走。
 */

export interface Pt {
  x: number;
  y: number;
}

export const ROOM = { x: 128, y: 104, wallHeight: 58 } as const;

/** 家具在地面上的占地矩形，走道不允许穿过。 */
export interface Footprint {
  id: string;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export const FURNITURE: readonly Footprint[] = [
  { id: "bookshelf", x0: 4, y0: 0, x1: 22, y1: 8 },
  { id: "books", x0: 24, y0: 4, x1: 29, y1: 8 },
  { id: "racks", x0: 48, y0: 0, x1: 70, y1: 9 },
  { id: "cooler", x0: 76, y0: 3, x1: 81, y1: 8 },
  { id: "cones", x0: 90, y0: 4, x1: 100, y1: 8 },
  { id: "coat-rack", x0: 102, y0: 3, x1: 106, y1: 7 },
  { id: "corner-plant", x0: 122, y0: 1, x1: 128, y1: 8 },
  { id: "boss-desk", x0: 102, y0: 12, x1: 122, y1: 21 },
  { id: "monstera", x0: 121, y0: 24, x1: 127, y1: 30 },
  { id: "pc", x0: 5, y0: 40, x1: 11, y1: 52 },
  { id: "bin", x0: 6, y0: 54, x1: 12, y1: 60 },
  { id: "aquarium", x0: 0, y0: 58, x1: 6, y1: 74 },
  { id: "desk-1", x0: 14, y0: 38, x1: 54, y1: 56 },
  { id: "chair-1", x0: 28.5, y0: 57.5, x1: 39.5, y1: 67.4 },
  { id: "desk-2", x0: 56, y0: 38, x1: 96, y1: 56 },
  { id: "chair-2", x0: 70.5, y0: 57.5, x1: 81.5, y1: 67.4 },
  { id: "desk-3", x0: 56, y0: 72, x1: 96, y1: 90 },
  { id: "chair-3", x0: 70.5, y0: 91.5, x1: 81.5, y1: 101.4 },
  { id: "review-table", x0: 104, y0: 52, x1: 120, y1: 64 },
  { id: "review-stool", x0: 109, y0: 67, x1: 114, y1: 72 },
  { id: "floor-lamp", x0: 6, y0: 70, x1: 9, y1: 73 },
  { id: "arcade", x0: 40, y0: 70, x1: 47, y1: 76 },
  { id: "tv", x0: 0, y0: 78, x1: 6, y1: 99 },
  { id: "sofa", x0: 24, y0: 73, x1: 36.5, y1: 101 },
  { id: "cushion", x0: 7, y0: 88, x1: 13, y1: 94 },
  { id: "beanbag", x0: 41, y0: 90, x1: 51, y1: 100 },
  { id: "dock", x0: 53, y0: 96, x1: 57, y1: 100 },
  { id: "fridge", x0: 124, y0: 82, x1: 128, y1: 91 },
  { id: "tea-bar", x0: 102, y0: 94, x1: 124, y1: 102 },
];

export interface WalkNode {
  id: string;
  at: Pt;
}

/**
 * 走道节点：只放在空地上，边是直线段。坐标是机器人脚下的中心点。
 * 左侧 PC、纸篓、鱼缸之间太窄，后排走道只从东侧（会议桌旁）连到前排。
 */
export const NODES: readonly WalkNode[] = [
  { id: "back-west", at: { x: 12.5, y: 32 } },
  { id: "back-mid", at: { x: 52, y: 30 } },
  { id: "back-east", at: { x: 84, y: 32 } },
  { id: "east-top", at: { x: 100, y: 32 } },
  { id: "east-review", at: { x: 100, y: 46 } },
  { id: "east-mid", at: { x: 100, y: 69 } },
  { id: "east-tea", at: { x: 100, y: 90 } },
  { id: "mid-west", at: { x: 16, y: 69 } },
  { id: "mid-desk-1", at: { x: 34, y: 69 } },
  { id: "mid-arcade", at: { x: 48, y: 69 } },
  { id: "mid-desk-2", at: { x: 76, y: 69 } },
  { id: "lounge", at: { x: 16, y: 84 } },
  { id: "front-west", at: { x: 14, y: 103 } },
  { id: "front-beanbag", at: { x: 42, y: 103 } },
  { id: "front-desk-3", at: { x: 70, y: 103 } },
  { id: "front-east", at: { x: 100, y: 103 } },
];

export const EDGES: readonly (readonly [string, string])[] = [
  ["back-west", "back-mid"],
  ["back-mid", "back-east"],
  ["back-east", "east-top"],
  ["east-top", "east-review"],
  ["east-review", "east-mid"],
  ["east-mid", "east-tea"],
  ["east-tea", "front-east"],
  ["mid-west", "mid-desk-1"],
  ["mid-desk-1", "mid-arcade"],
  ["mid-arcade", "mid-desk-2"],
  ["mid-desk-2", "east-mid"],
  ["mid-west", "lounge"],
  ["lounge", "front-west"],
  ["front-west", "front-beanbag"],
  ["front-beanbag", "front-desk-3"],
  ["front-desk-3", "front-east"],
];

export type SpotKind = "desk" | "plan" | "validate" | "review" | "queue" | "rest";
/** 渲染层据此选贴图：背对坐（工位）、正面坐、侧坐（沙发）、站、睡。 */
export type SpotPose = "sit-back" | "sit-front" | "sit-side" | "stand" | "sleep";

export interface Spot {
  id: string;
  kind: SpotKind;
  at: Pt;
  /** 机器人脚下的高度（坐下时是椅面高度）。 */
  z: number;
  pose: SpotPose;
  /** 进出这个点位时连接的走道节点。 */
  node: string;
  /** 给读屏与面板用的地点名。 */
  label: string;
  /** 绘制深度（与贴图生成脚本里同一位置的排序键一致），决定哪些家具会盖住机器人。 */
  depth: number;
}

/** 顺序有意义：同类点位按数组顺序优先使用。 */
export const SPOTS: readonly Spot[] = [
  { id: "desk-1", kind: "desk", at: { x: 34, y: 62 }, z: 8.6, pose: "sit-back", node: "mid-desk-1", label: "1 号工位", depth: 96 },
  { id: "desk-2", kind: "desk", at: { x: 76, y: 62 }, z: 8.6, pose: "sit-back", node: "mid-desk-2", label: "2 号工位", depth: 138 },
  { id: "desk-3", kind: "desk", at: { x: 76, y: 96 }, z: 8.6, pose: "sit-back", node: "front-desk-3", label: "3 号工位", depth: 172 },
  { id: "whiteboard", kind: "plan", at: { x: 11.25, y: 29 }, z: 0, pose: "stand", node: "back-west", label: "白板", depth: 36 },
  { id: "racks", kind: "validate", at: { x: 63.25, y: 16 }, z: 0, pose: "stand", node: "back-mid", label: "实验架", depth: 78 },
  { id: "review-seat", kind: "review", at: { x: 111.25, y: 49 }, z: 8, pose: "sit-front", node: "east-review", label: "会议桌", depth: 157 },
  { id: "queue-1", kind: "queue", at: { x: 100.25, y: 28 }, z: 0, pose: "stand", node: "east-top", label: "你的桌前", depth: 124.5 },
  { id: "queue-2", kind: "queue", at: { x: 93.25, y: 32 }, z: 0, pose: "stand", node: "back-east", label: "你的桌前", depth: 122 },
  { id: "queue-3", kind: "queue", at: { x: 86.25, y: 36 }, z: 0, pose: "stand", node: "back-east", label: "你的桌前", depth: 118 },
  { id: "sofa-1", kind: "rest", at: { x: 29, y: 82 }, z: 7.6, pose: "sit-side", node: "lounge", label: "沙发", depth: 110 },
  { id: "sofa-2", kind: "rest", at: { x: 29, y: 92 }, z: 7.6, pose: "sit-side", node: "lounge", label: "沙发", depth: 120 },
  { id: "beanbag", kind: "rest", at: { x: 46.25, y: 96 }, z: 4.5, pose: "sleep", node: "front-beanbag", label: "豆袋", depth: 142 },
  { id: "tea", kind: "rest", at: { x: 99.25, y: 92 }, z: 0, pose: "stand", node: "east-tea", label: "茶水吧", depth: 186 },
  { id: "arcade", kind: "rest", at: { x: 47.75, y: 83 }, z: 0, pose: "stand", node: "front-beanbag", label: "街机", depth: 130 },
];

export function spotById(id: string): Spot | undefined {
  return SPOTS.find((spot) => spot.id === id);
}

export function nodeById(id: string): WalkNode | undefined {
  return NODES.find((node) => node.id === id);
}

/** 线段是否穿过某块家具占地（端点贴边不算）。用于测试走道数据。 */
export function segmentHitsFootprint(a: Pt, b: Pt, f: Footprint, samples = 64): boolean {
  for (let i = 0; i <= samples; i++) {
    const t = i / samples;
    const x = a.x + (b.x - a.x) * t;
    const y = a.y + (b.y - a.y) * t;
    if (x > f.x0 && x < f.x1 && y > f.y0 && y < f.y1) return true;
  }
  return false;
}
