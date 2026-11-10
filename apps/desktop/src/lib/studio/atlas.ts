import manifestJson from "@/assets/studio/studio-manifest.json";
import { botFamily, type SquadMember } from "@/components/pixel/PixelBot";
import type { PoseKey } from "./motion";

/** atlas 里的一块：[x, y, w, h]。 */
export type Rect = [number, number, number, number];

export interface RobotFrame {
  /** 锚点（脚下点在屏幕上的位置）在贴图内的偏移。 */
  ox: number;
  oy: number;
  w: number;
  h: number;
  rect: Rect;
}

/** 换色层：同一个锚点，base = 机身色，light = 面板色（灰色画的）。 */
export interface PaintFrame {
  base: RobotFrame | null;
  light: RobotFrame | null;
}

/** 主人自定义用的额外贴图（Agent 档案）。 */
export interface CustomAtlas {
  /** 天线造型 → 姿势 → 帧。 */
  paint: Record<string, Partial<Record<PoseKey, PaintFrame[]>>>;
  /** 配件 → 姿势 → 帧（看不见的姿势是 null）。 */
  acc: Record<string, Partial<Record<PoseKey, Array<RobotFrame | null>>>>;
  /** 机器人家族 → 天线造型。 */
  antenna: Record<string, string>;
  grey: { base: number; light: number };
}

export interface ThemeAtlas {
  /** 家具贴图，按 key 排序；用于盖住站在它后面的机器人。 */
  items: { key: number; x: number; y: number; rect: Rect }[];
  /** 有人坐时亮起的屏幕，4 帧。 */
  desks: Record<string, { x: number; y: number; frames: Rect[] }>;
  robots: Record<string, Record<PoseKey, RobotFrame[]>>;
  you: RobotFrame;
  custom: CustomAtlas;
}

export interface StudioManifest {
  W: number;
  H: number;
  OX: number;
  OY: number;
  frames: number;
  day: ThemeAtlas;
  night: ThemeAtlas;
}

export const MANIFEST = manifestJson as unknown as StudioManifest;

/** 地面坐标 → 原尺寸像素坐标（与生成脚本同一投影：2:1 等距斜俯视）。 */
export function project(x: number, y: number, z = 0): [number, number] {
  return [MANIFEST.OX + 2 * (x - y), MANIFEST.OY + (x + y) - z];
}

/** Provider → 贴图里的机器人家族；认不出的用中性色。 */
export function atlasFamily(member: string): string {
  const fam = botFamily(member as SquadMember);
  return fam in MANIFEST.day.robots ? fam : "neutral";
}
