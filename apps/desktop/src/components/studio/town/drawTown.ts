import type { ProjectSummary } from "@/generated/bindings";
import type { ProjectKind, ProjectLook as HouseLook, Tier } from "@/lib/studio/buildings";
import type { CityLayout } from "@/lib/studio/cityLayout";
import type { ThemeAtlas } from "@/lib/studio/atlas";
import { parseHex } from "@/lib/studio/paint";
import { drawRobotSprite, type RobotLooks } from "@/components/studio/drawRobot";
import { tinted } from "@/components/studio/paintCache";
import { spriteOrigin, type PoseKey } from "@/lib/studio/motion";
import { residents, type ProjectLook } from "@/lib/studio/townResidents";
import { CREW_SPOTS, constructionCrew, hammerBeat, weedsFor } from "@/lib/studio/townEvents";
import { TOWN, blockOrigin, buildingSprite, paintSprite, tileKey, townProject, type TownGeometry, type TownSprite, type TownTheme } from "@/lib/studio/townAtlas";

export interface TownProject {
  id: string;
  name: string;
  kind: ProjectKind;
  tier: Tier;
  look: ProjectLook;
  /** 主人的装修：屋顶颜色、门牌样式、院子摆件。 */
  house?: HouseLook;
  summary: ProjectSummary | undefined;
}

export interface TownImages {
  tiles: HTMLImageElement;
  sprites: HTMLImageElement;
  robots: HTMLImageElement;
}

export interface TownFrame {
  ctx: CanvasRenderingContext2D;
  geometry: TownGeometry;
  city: CityLayout;
  projects: ReadonlyMap<string, TownProject>;
  theme: TownTheme;
  robots: ThemeAtlas;
  images: TownImages;
  time: number;
  reduce: boolean;
  /** "day" | "night"：换色缓存按它区分 atlas。 */
  themeKey: string;
  /** Agent 档案里的颜色和配件。 */
  looks: RobotLooks;
  /** 夜里：没活干的机器人回屋睡觉，屋顶冒 z。 */
  sleepy?: boolean;
}

/** 画在画布像素上的锚点，给 HTML 名牌 / 设施标签定位。 */
export interface TownAnchor {
  key: string;
  x: number;
  y: number;
}

const DOOR = { x: 48, y: 56 };

function blit(ctx: CanvasRenderingContext2D, img: HTMLImageElement, s: TownSprite, ox: number, oy: number) {
  const [sx, sy, w, h] = s.rect;
  ctx.drawImage(img, sx, sy, w, h, Math.round(ox + s.dx), Math.round(oy + s.dy), w, h);
}

function frameOf(time: number, fps: number, count: number, reduce: boolean): number {
  // 时钟刚起步时可能是负数，取模要落回 [0, count)
  return reduce || count <= 1 ? 0 : ((Math.floor(time * fps) % count) + count) % count;
}

/**
 * 画一整帧小镇：街区按由远到近的顺序画，每个街区先画地面贴图，再画房子、状态装饰和院子里的机器人。
 * 返回项目名牌和公共设施标签的位置（画布像素）。
 */
export function drawTown(f: TownFrame): { signs: TownAnchor[]; places: TownAnchor[] } {
  const { ctx, geometry: g, city, theme, images, time, reduce } = f;
  const B = TOWN.block;
  const signs: TownAnchor[] = [];
  const places: TownAnchor[] = [];
  const cells = [...city.cells].sort((a, b) => a.col + a.row - (b.col + b.row) || a.col - b.col);
  const tileFrame = frameOf(time, 1.6, TOWN.tileFrames, reduce);

  for (const cell of cells) {
    const [ox, oy] = blockOrigin(g, cell);
    const tile = theme.tiles[tileKey(cell)]?.[tileFrame];
    if (tile) blit(ctx, images.tiles, tile, ox, oy);

    if (cell.type !== "project") {
      const [px, py] = townProject(g, cell.col * B + 48, cell.row * B + 48, 0);
      places.push({ key: cell.type === "facility" ? cell.facility : cell.filler, x: px, y: py });
      continue;
    }
    const p = f.projects.get(cell.projectId);
    if (!p) continue;
    const ov = theme.overlays;
    if (p.look.leaves) blit(ctx, images.sprites, ov.leaves[0], ox, oy);
    if (p.look.weeds) drawWeeds(f, p.id, cell.col * B, cell.row * B);
    let roofTop = oy;
    if (p.look.construction) {
      blit(ctx, images.sprites, ov.construction[frameOf(time, 1.2, ov.construction.length, reduce)], ox, oy);
      // 施工队：围着工地抡锤，锤下去迸火星
      constructionCrew(p.summary).forEach((agent, i) => {
        const [cx, cy] = CREW_SPOTS[i];
        const beat = hammerBeat(time, i, reduce);
        drawRobot(f, agent, "stand-work", cell.col * B + cx, cell.row * B + cy, beat.hop);
        if (beat.spark) drawSpark(f, cell.col * B + cx, cell.row * B + cy);
      });
    } else {
      const b = buildingSprite(theme, p.kind, p.tier, p.look.lit);
      blit(ctx, images.sprites, b, ox, oy);
      const roof = parseHex(p.house?.roof);
      const paint = roof && paintSprite(theme, p.kind, p.tier, p.look.lit);
      if (roof && paint) {
        const c = tinted(images.sprites, paint.rect, roof, TOWN.paintGrey, `town-${f.themeKey}`);
        if (c) ctx.drawImage(c, Math.round(ox + paint.dx), Math.round(oy + paint.dy));
      }
      roofTop = oy + b.dy;
      if (p.look.smoke) {
        const puff = ov.smoke[frameOf(time, 4, ov.smoke.length, reduce)];
        for (const [ax, ay] of theme.smoke[p.kind]?.[p.tier - 1] ?? []) blit(ctx, images.sprites, puff, ox + ax, oy + ay);
      }
    }
    if (p.look.flag) blit(ctx, images.sprites, ov.flag[frameOf(time, 2, ov.flag.length, reduce)], ox, oy);

    // the owner's yard decorations, each at its own spot
    for (const item of p.house?.yard ?? []) {
      const frames = theme.yard?.[item];
      if (frames?.length) blit(ctx, images.sprites, frames[frameOf(time, 2, frames.length, reduce)], ox, oy);
    }

    // robots in the yard: as many as this project's assigned agents
    const yard = residents(p.id, p.summary, DOOR, { x: TOWN.mailbox[0], y: TOWN.mailbox[1] }, TOWN.yard, time, reduce);
    yard.sort((a, b) => a.y - b.y);
    const outside = f.sleepy ? yard.filter((r) => r.pose === "stand-work" || r.pose === "stand-hand") : yard;
    for (const r of outside) drawRobot(f, r.agent, r.pose, cell.col * B + r.x, cell.row * B + r.y);
    if (outside.length < yard.length) drawZzz(f, cell.col * B + DOOR.x, cell.row * B + DOOR.y, p.id);

    if (p.look.cloud) {
      const c = ov.cloud[frameOf(time, 4, ov.cloud.length, reduce)];
      const shift = roofTop - 10 - (oy + c.dy + c.h);
      blit(ctx, images.sprites, c, ox, oy + Math.min(0, shift));
    }
    const [sx, sy] = townProject(g, cell.col * B + TOWN.sign[0], cell.row * B + TOWN.sign[1], TOWN.sign[2]);
    signs.push({ key: p.id, x: sx, y: sy });
  }
  return { signs, places };
}

function drawRobot(f: TownFrame, agent: string, pose: PoseKey, x: number, y: number, hop = 0) {
  const frame = frameOf(f.time, pose.startsWith("walk") ? 6 : 3, 2, f.reduce);
  const o = spriteOrigin({ id: agent, spot: "", pos: { x, y }, path: [], facing: "front" }, pose);
  const [px, py] = townProject(f.geometry, o.x, o.y, hop);
  drawRobotSprite({ ctx: f.ctx, img: f.images.robots, atlas: f.robots, imgKey: `studio-${f.themeKey}` }, agent, pose, frame, px, py, f.looks.get(agent));
}

/** 杂草和火星的颜色取主题 token，按昼夜缓存。 */
const colorCache = new Map<string, { blade: string; tip: string; spark: string; sparkHot: string }>();
function colors(themeKey: string) {
  let c = colorCache.get(themeKey);
  if (!c) {
    const css = getComputedStyle(document.documentElement);
    const v = (n: string) => css.getPropertyValue(n).trim();
    const night = themeKey === "night";
    c = {
      blade: v(night ? "--color-town-weed-night" : "--color-town-weed"),
      tip: v(night ? "--color-town-weed" : "--color-town-weed-tip"),
      spark: v("--color-bot-kimi"),
      sparkHot: v("--color-bot-kimi-light"),
    };
    colorCache.set(themeKey, c);
  }
  return c;
}

/** 一个月没动：院子里冒出一丛丛像素杂草，随风轻轻摆。 */
function drawWeeds(f: TownFrame, projectId: string, bx: number, by: number) {
  const c = colors(f.themeKey);
  for (const w of weedsFor(projectId)) {
    const [px, py] = townProject(f.geometry, bx + w.x, by + w.y);
    const sway = f.reduce ? 0 : Math.round(Math.sin(f.time * 1.6 + w.phase) * 0.6);
    const base = Math.round(py);
    w.blades.forEach((h, i) => {
      const x = Math.round(px) + i - Math.floor(w.blades.length / 2);
      const lean = i % 2 ? sway : -sway;
      f.ctx.fillStyle = c.blade;
      f.ctx.fillRect(x, base - h + 2, 1, h - 1);
      f.ctx.fillStyle = c.tip;
      f.ctx.fillRect(x + lean, base - h, 1, 2);
    });
    if (w.flower) {
      const tallest = Math.max(...w.blades);
      f.ctx.fillStyle = c.spark;
      f.ctx.fillRect(Math.round(px) - 1 + sway, base - tallest - 2, 2, 2);
    }
  }
}

/** 锤子落下的一瞬：机器人身前迸出几颗火星。 */
function drawSpark(f: TownFrame, x: number, y: number) {
  const c = colors(f.themeKey);
  const [px, py] = townProject(f.geometry, x, y, 6);
  const sx = Math.round(px) + 5, sy = Math.round(py) - 4;
  f.ctx.fillStyle = c.sparkHot;
  f.ctx.fillRect(sx, sy, 1, 1);
  f.ctx.fillStyle = c.spark;
  for (const [dx, dy] of [[-2, -1], [2, -2], [1, 1], [-1, 2]]) f.ctx.fillRect(sx + dx, sy + dy, 1, 1);
}

/** 屋里有人在睡：门口上方飘起三个越来越大的像素 z。 */
function drawZzz(f: TownFrame, x: number, y: number, seed: string) {
  const c = colors(f.themeKey);
  const offset = (seed.length * 0.37) % 1;
  for (let i = 0; i < 3; i++) {
    const t = f.reduce ? i / 3 : (f.time * 0.35 + offset + i / 3) % 1;
    const [px, py] = townProject(f.geometry, x, y, 40 + t * 22);
    const s = 1 + Math.floor(t * 2); // 1–2 像素一笔，4×4 的 Z
    const zx = Math.round(px + 14 + t * 10), zy = Math.round(py);
    f.ctx.globalAlpha = 1 - t * 0.7;
    f.ctx.fillStyle = c.sparkHot;
    f.ctx.fillRect(zx, zy, 4 * s, s);
    f.ctx.fillRect(zx + 2 * s, zy + s, s, s);
    f.ctx.fillRect(zx + s, zy + 2 * s, s, s);
    f.ctx.fillRect(zx, zy + 3 * s, 4 * s, s);
  }
  f.ctx.globalAlpha = 1;
}
