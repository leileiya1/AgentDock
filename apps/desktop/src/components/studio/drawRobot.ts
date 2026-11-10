import type { AgentProfile } from "@/generated/bindings";
import { atlasFamily, type RobotFrame, type ThemeAtlas } from "@/lib/studio/atlas";
import type { PoseKey } from "@/lib/studio/motion";
import { lighten, parseHex, type Rgb } from "@/lib/studio/paint";
import { tinted } from "@/components/studio/paintCache";

/** 一位 Agent 在像素画里的样子（来自 Agent 档案）。 */
export interface RobotLook {
  color: Rgb | null;
  accessory: string | null;
}
export type RobotLooks = ReadonlyMap<string, RobotLook>;

export function robotLooks(profiles: readonly AgentProfile[] | undefined): RobotLooks {
  const out = new Map<string, RobotLook>();
  for (const p of profiles ?? []) {
    const color = parseHex(p.color);
    const accessory = p.accessory && p.accessory !== "none" ? p.accessory : null;
    if (color || accessory) out.set(p.agent, { color, accessory });
  }
  return out;
}

export interface RobotDraw {
  ctx: CanvasRenderingContext2D;
  img: HTMLImageElement;
  atlas: ThemeAtlas;
  /** 区分白天 / 夜间 atlas，用作换色缓存的 key。 */
  imgKey: string;
}

function blitFrame(d: RobotDraw, fr: RobotFrame, ax: number, ay: number) {
  d.ctx.drawImage(d.img, fr.rect[0], fr.rect[1], fr.rect[2], fr.rect[3], Math.round(ax - fr.ox), Math.round(ay - fr.oy), fr.w, fr.h);
}

function blitTinted(d: RobotDraw, fr: RobotFrame | null, color: Rgb, grey: number, ax: number, ay: number) {
  if (!fr) return;
  const c = tinted(d.img, fr.rect, color, grey, d.imgKey);
  if (c) d.ctx.drawImage(c, Math.round(ax - fr.ox), Math.round(ay - fr.oy));
}

/**
 * 画一个机器人：家族贴图 → （选了颜色时）机身和面板换色层 → （选了配件时）配件层。
 * (ax, ay) 是脚下锚点的像素坐标；返回贴图占的框，给遮挡判断用。
 */
export function drawRobotSprite(d: RobotDraw, member: string, pose: PoseKey, frame: number, ax: number, ay: number, look?: RobotLook) {
  const fam = atlasFamily(member);
  const frames = d.atlas.robots[fam]?.[pose];
  if (!frames?.length) return null;
  const i = ((frame % frames.length) + frames.length) % frames.length;
  const fr = frames[i];
  if (!fr) return null;
  blitFrame(d, fr, ax, ay);
  const custom = d.atlas.custom;
  if (look?.color && custom) {
    const paint = custom.paint[custom.antenna[fam] ?? ""]?.[pose]?.[i];
    if (paint) {
      blitTinted(d, paint.base, look.color, custom.grey.base, ax, ay);
      blitTinted(d, paint.light, lighten(look.color), custom.grey.light, ax, ay);
    }
  }
  if (look?.accessory && custom) {
    const acc = custom.acc[look.accessory]?.[pose]?.[i];
    if (acc) blitFrame(d, acc, ax, ay);
  }
  return { x: Math.round(ax - fr.ox), y: Math.round(ay - fr.oy), w: fr.w, h: fr.h };
}
