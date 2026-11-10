import { useEffect, useMemo, useRef, useState } from "react";
import type { TaskSummary } from "@/generated/bindings";
import dayBgUrl from "@/assets/studio/studio-day-bg.png";
import nightBgUrl from "@/assets/studio/studio-night-bg.png";
import dayAtlasUrl from "@/assets/studio/studio-day-atlas.png";
import nightAtlasUrl from "@/assets/studio/studio-night-atlas.png";
import { memberActivities, type Activity } from "@/lib/studio/activity";
import { assignSpots, type Placement } from "@/lib/studio/assign";
import { spotById } from "@/lib/studio/layout";
import { depthOf, poseFor, spriteOrigin, stepActor, syncActors, type Actor } from "@/lib/studio/motion";
import { MANIFEST, atlasFamily, project, type RobotFrame, type ThemeAtlas } from "@/lib/studio/atlas";
import { toScreen } from "@/lib/studio/camera";
import { useCanvasCamera } from "@/components/studio/useCanvasCamera";
import { ThemeSwitch, ZoomControls, type ThemeChoice } from "@/components/studio/SceneControls";
import { useSky } from "@/hooks/useSky";
import { SkyChip, drawTint } from "@/components/studio/skyTint";
import { memberShortLabel, type SquadMember } from "@/components/pixel/PixelBot";
import { cn } from "@/lib/utils";
import { useAgentProfiles } from "@/hooks/useStudio";
import { drawRobotSprite, robotLooks } from "@/components/studio/drawRobot";
import { RobotCard } from "@/components/studio/RobotCard";
import { useRobotPointer } from "@/components/studio/useRobotPointer";
import type { RobotBox } from "@/lib/studio/robotHit";
import { useParams } from "react-router-dom";
import { useUiStore } from "@/stores/uiStore";
import { useCelebrateStore } from "@/stores/celebrateStore";
import { jumpOffset } from "@/components/celebrate/fireworksModel";

const { W, H } = MANIFEST;
const SCENE = { w: W, h: H };
// default view: the house fills the frame, with a ring of garden around it
const HOME = (() => {
  const [cx, cy] = project(64, 52, 20);
  return { center: { x: cx, y: cy }, zoom: 1.3 };
})();

function viewportFor(width: number): { w: number; h: number } {
  // 画面比例跟场景一致（场景四周是花园），默认铺满时屋子和一圈花园都在画面里
  return { w: Math.max(320, width), h: Math.round(Math.min(680, Math.max(380, (width / W) * H))) };
}
const YOU_AT = { x: 108, y: 5, depth: 108 };

const VERB: Record<Exclude<Activity, "idle">, string> = {
  plan: "拆步骤",
  develop: "写代码",
  validate: "跑测试",
  review: "读改动",
  waiting: "等你处理",
};
const REST_VERB: Record<string, string> = { "sofa-1": "看电视", "sofa-2": "看电视", beanbag: "打盹", tea: "喝茶", arcade: "打游戏" };

/** 档案里起的名字（key = agent），没起就用默认短名。 */
type Nicknames = ReadonlyMap<string, string>;
function name(id: string, nicknames: Nicknames = new Map()): string {
  const nick = nicknames.get(id);
  if (nick) return nick;
  return id === "runner" ? "验证机" : memberShortLabel(id as SquadMember);
}

function bubbleText(p: Placement, actor: Actor | undefined, nicknames: Nicknames, phrases: Nicknames): string {
  const who = name(p.id, nicknames);
  const phrase = phrases.get(p.id);
  if (phrase && p.activity === "idle" && !actor?.path.length) return `${who} · 「${phrase}」`;
  if (actor?.path.length) return `${who} · 去${spotById(actor.spot)?.label ?? "工位"}`;
  if (p.activity === "idle") return `${who} · ${REST_VERB[p.spot ?? ""] ?? "歇着"}`;
  return `${who} · ${VERB[p.activity]}`;
}

/** 名片上「正在干嘛」那半句。 */
function doingText(p: Placement, actor: Actor | undefined): string {
  if (actor?.path.length) return `去${spotById(actor.spot)?.label ?? "工位"}`;
  if (p.activity === "idle") return REST_VERB[p.spot ?? ""] ?? "歇着";
  return VERB[p.activity];
}

function bubbleTone(p: Placement, actor: Actor | undefined): string {
  if (actor?.path.length) return p.activity === "idle" ? "bg-raised text-t1 ring-1 ring-line-strong" : "bg-status-running text-white";
  if (p.activity === "waiting") return "bg-status-human text-white";
  if (p.activity === "idle") return "bg-raised text-t1 ring-1 ring-line-strong";
  return "bg-status-running text-white";
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = src;
  });
}

function prefersReducedMotion(): boolean {
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

/**
 * 像素小屋视图。机器人的位置和动作只来自任务状态（活动推导 → 分配点位 → 沿走道行走），
 * 画面只是另一种看法；点气泡打开对应任务。窗口不在前台时不画，减少动态效果时不走路、不播帧。
 */
export function StudioRoom({ tasks, onOpenTask }: { tasks: TaskSummary[]; onOpenTask: (taskId: string) => void }) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const bubbleRefs = useRef(new Map<string, HTMLElement>());
  const prevSpots = useRef(new Map<string, string>());
  const actorsRef = useRef(new Map<string, Actor>());
  const [choice, setChoice] = useState<ThemeChoice>("auto");
  const [images, setImages] = useState<Record<string, HTMLImageElement> | null>(null);
  // who is walking right now — re-render bubbles only when this set changes
  const [, setWalking] = useState("");
  // 点开名片的机器人
  const [selected, setSelected] = useState<string | null>(null);
  const selectedRef = useRef<string | null>(null);
  selectedRef.current = selected;
  const cardRef = useRef<HTMLDivElement>(null);
  const boxesRef = useRef<RobotBox[]>([]);
  const { projectId } = useParams();
  const projectIdRef = useRef(projectId);
  projectIdRef.current = projectId;
  const openNewTask = useUiStore((s) => s.openNewTask);
  const reduce = useMemo(prefersReducedMotion, []);
  const camera = useCanvasCamera(wrapRef, canvasRef, SCENE, viewportFor, HOME, reduce);
  const { view, camRef } = camera;
  const sky = useSky(choice);
  const theme = sky.theme;
  const skyRef = useRef(sky);
  skyRef.current = sky;
  const profiles = useAgentProfiles();
  const looks = useMemo(() => robotLooks(profiles.data), [profiles.data]);
  const looksRef = useRef(looks);
  looksRef.current = looks;
  const phrases = useMemo<Nicknames>(
    () => new Map((profiles.data ?? []).filter((p) => p.catchphrase).map((p) => [p.agent, p.catchphrase!])),
    [profiles.data]
  );
  const nicknames = useMemo<Nicknames>(
    () => new Map((profiles.data ?? []).filter((p) => p.nickname).map((p) => [p.agent, p.nickname!])),
    [profiles.data]
  );

  useRobotPointer({ canvasRef, camRef, view, boxesRef, onPick: setSelected });
  useEffect(() => {
    if (!selected) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setSelected(null);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selected]);

  const placements = useMemo(() => {
    const out = assignSpots(memberActivities(tasks), prevSpots.current);
    prevSpots.current = new Map(out.filter((p) => p.spot).map((p) => [p.id, p.spot!]));
    return out;
  }, [tasks]);
  const placementsRef = useRef(placements);
  placementsRef.current = placements;

  useEffect(() => {
    actorsRef.current = syncActors(actorsRef.current, placements, !reduce);
  }, [placements, reduce]);

  useEffect(() => {
    let alive = true;
    Promise.all([loadImage(dayBgUrl), loadImage(nightBgUrl), loadImage(dayAtlasUrl), loadImage(nightAtlasUrl)])
      .then(([db, nb, da, na]) => alive && setImages({ dayBg: db, nightBg: nb, dayAtlas: da, nightAtlas: na }))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !images) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const bg = images[theme === "day" ? "dayBg" : "nightBg"];
    const atlasImg = images[theme === "day" ? "dayAtlas" : "nightAtlas"];
    const atlas: ThemeAtlas = MANIFEST[theme];
    let raf = 0;
    let last = performance.now();
    let time = 0;

    const dpr = Math.max(1, window.devicePixelRatio || 1);
    canvas.width = Math.round(view.w * dpr);
    canvas.height = Math.round(view.h * dpr);
    const night = theme === "night";
    const css = getComputedStyle(document.documentElement);
    const token = (name: string) => css.getPropertyValue(name).trim();
    const backTop = token(night ? "--color-studio-night" : "--color-studio-day");
    const backBottom = token(night ? "--color-studio-night-deep" : "--color-studio-day-deep");
    const gridColor = token(night ? "--color-studio-grid-night" : "--color-studio-grid-day");

    const draw = () => {
      const cam = camRef.current;
      // backdrop: fills whatever the room does not cover, with a faint floor grid that moves with the room
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const g = ctx.createLinearGradient(0, 0, 0, view.h);
      g.addColorStop(0, backTop);
      g.addColorStop(1, backBottom);
      ctx.fillStyle = g;
      ctx.fillRect(0, 0, view.w, view.h);
      ctx.setTransform(dpr * cam.z, 0, 0, dpr * cam.z, Math.round(dpr * (view.w / 2 - cam.x * cam.z)), Math.round(dpr * (view.h / 2 - cam.y * cam.z)));
      ctx.imageSmoothingEnabled = false;
      ctx.fillStyle = gridColor;
      const x0 = cam.x - view.w / 2 / cam.z, y0 = cam.y - view.h / 2 / cam.z;
      const step = 16;
      for (let gy = Math.floor(y0 / step) * step; gy < y0 + view.h / cam.z + step; gy += step) {
        for (let gx = Math.floor(x0 / (step * 2)) * step * 2; gx < x0 + view.w / cam.z + step * 2; gx += step * 2) {
          ctx.fillRect(gx + ((gy / step) % 2 ? step : 0), gy, 1, 1);
        }
      }
      const frame = reduce ? 0 : Math.floor(time * 2.5) % MANIFEST.frames;
      ctx.drawImage(bg, frame * W, 0, W, H, 0, 0, W, H);

      const actors = [...actorsRef.current.values()];
      const byId = new Map(placementsRef.current.map((p) => [p.id, p]));

      // lit screens where someone is sitting
      for (const a of actors) {
        const lit = !a.path.length ? atlas.desks[a.spot] : undefined;
        if (lit) {
          const r = lit.frames[frame];
          ctx.drawImage(atlasImg, r[0], r[1], r[2], r[3], lit.x, lit.y, r[2], r[3]);
        }
      }

      type Entry = { k: number; draw: () => void; box?: [number, number, number, number] };
      const entries: Entry[] = [];
      const labels: { id: string; el: HTMLElement; cx: number; top: number }[] = [];
      const boxes: RobotBox[] = [];
      const robot = { ctx, img: atlasImg, atlas, imgKey: `studio-${theme}` };
      const sprite = (fr: RobotFrame, x: number, y: number) => () => ctx.drawImage(atlasImg, fr.rect[0], fr.rect[1], fr.rect[2], fr.rect[3], x, y, fr.w, fr.h);

      // 合并庆祝：本项目（或彩蛋时全员）的开发者原地蹦几下
      const party = useCelebrateStore.getState().queue.filter((c) => !c.projectId || c.projectId === projectIdRef.current);
      const wall = Date.now();
      for (const a of actors) {
        const p = byId.get(a.id);
        if (!p) continue;
        const hop = Math.max(0, ...party.filter((c) => !c.agent || c.agent === a.id).map((c) => (reduce ? 0 : jumpOffset(wall - c.at))));
        const pose = poseFor(a, p.activity);
        const frames = atlas.robots[atlasFamily(a.id)][pose];
        const speed = a.path.length ? 6 : pose === "sit-side" || pose === "sleep" ? 1.5 : 3;
        const fi = reduce ? 0 : Math.floor(time * speed) % frames.length;
        const fr = frames[fi];
        const o = spriteOrigin(a, pose);
        const [px, py] = project(o.x, o.y);
        const x = Math.round(px - fr.ox), y = Math.round(py - fr.oy) - hop;
        const look = looksRef.current.get(a.id);
        entries.push({ k: depthOf(a), draw: () => void drawRobotSprite(robot, a.id, pose, fi, px, py - hop, look), box: [x, y, fr.w, fr.h] });
        boxes.push({ id: a.id, depth: depthOf(a), x, y, w: fr.w, h: fr.h });
        const bubble = bubbleRefs.current.get(a.id);
        if (bubble) {
          const sp = toScreen(cam, view, x + fr.w / 2, y - 3);
          labels.push({ id: a.id, el: bubble, cx: sp.x, top: sp.y });
        }
      }
      const you = atlas.you;
      const [yx, yy] = project(YOU_AT.x, YOU_AT.y);
      entries.push({ k: YOU_AT.depth, draw: sprite(you, Math.round(yx - you.ox), Math.round(yy - you.oy)), box: [yx - you.ox, yy - you.oy, you.w, you.h] });

      // furniture that stands in front of a robot is drawn again over it
      const robotBoxes = entries.filter((e) => e.box);
      const minDepth = Math.min(...robotBoxes.map((e) => e.k));
      for (const it of atlas.items) {
        if (it.key <= minDepth) continue;
        const [, , w, h] = it.rect;
        const hit = robotBoxes.some((e) => it.key > e.k && it.x < e.box![0] + e.box![2] && it.x + w > e.box![0] && it.y < e.box![1] + e.box![3] && it.y + h > e.box![1]);
        if (hit) entries.push({ k: it.key, draw: () => ctx.drawImage(atlasImg, it.rect[0], it.rect[1], w, h, it.x, it.y, w, h) });
      }
      entries.sort((a, b) => a.k - b.k);
      for (const e of entries) e.draw();
      drawTint(ctx, dpr, view, skyRef.current.tint);

      boxesRef.current = boxes;

      // 名片跟着选中的机器人走，放在它头顶；放不下就放脚下。它的气泡先藏起来。
      const card = cardRef.current;
      const sel = boxes.find((b) => b.id === selectedRef.current);
      if (card && sel) {
        const head = toScreen(cam, view, sel.x + sel.w / 2, sel.y);
        const feet = toScreen(cam, view, sel.x + sel.w / 2, sel.y + sel.h);
        const w = card.offsetWidth, h = card.offsetHeight;
        const left = Math.max(8, Math.min(view.w - w - 8, head.x - w / 2));
        const top = head.y - h - 6 >= 8 ? head.y - h - 6 : Math.min(view.h - h - 8, feet.y + 6);
        card.style.transform = `translate(${Math.round(left)}px, ${Math.round(top)}px)`;
        card.style.opacity = "1";
      }

      // speech bubbles: stack upward when two would overlap (e.g. the queue at your desk)
      labels.sort((a, b) => b.top - a.top);
      const placedBoxes: [number, number, number, number][] = [];
      for (const l of labels) {
        if (l.id === selectedRef.current) {
          l.el.style.opacity = "0";
          continue;
        }
        const w = l.el.offsetWidth, h = l.el.offsetHeight;
        let bottom = l.top;
        const left = l.cx - w / 2;
        for (let i = 0; i < 6; i++) {
          const clash = placedBoxes.find(([x0, y0, x1, y1]) => left < x1 && left + w > x0 && bottom - h < y1 && bottom > y0);
          if (!clash) break;
          bottom = clash[1] - 2;
        }
        placedBoxes.push([left, bottom - h, left + w, bottom]);
        l.el.style.transform = `translate(${Math.round(left)}px, ${Math.round(bottom - h)}px)`;
        l.el.style.opacity = "1";
      }
    };

    const loop = (now: number) => {
      raf = requestAnimationFrame(loop);
      if (document.hidden) {
        last = now;
        return;
      }
      const dt = Math.max(0, Math.min(0.1, (now - last) / 1000));
      last = now;
      time += dt;
      camera.step(dt);
      if (!reduce) {
        const next = new Map<string, Actor>();
        for (const [id, a] of actorsRef.current) next.set(id, stepActor(a, dt));
        actorsRef.current = next;
        setWalking([...next.values()].filter((a) => a.path.length).map((a) => a.id).join(","));
      }
      draw();
    };
    draw();
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
    // camera.step only reads refs
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [images, theme, view, reduce]);

  const placed = placements.filter((p) => p.spot);
  const picked = selected ? placed.find((p) => p.id === selected) : undefined;
  if (selected && !picked) queueMicrotask(() => setSelected(null));
  const overflow = placements.filter((p) => p.overflow);

  return (
    <section aria-label="像素小屋" className="flex flex-col gap-2">
      <div
        ref={wrapRef}
        tabIndex={0}
        onKeyDown={camera.onKeyDown}
        aria-label="小屋画面：滚轮或双指缩放，拖动平移，双击放大；键盘加减号缩放、方向键平移、0 复位"
        className={cn(
          "relative overflow-hidden rounded-card border border-line outline-none focus-visible:ring-2 focus-visible:ring-focus",
          theme === "night" ? "bg-studio-night" : "bg-studio-day"
        )}
        style={{ height: view.h }}
      >
        <canvas
          ref={canvasRef}
          className="block cursor-grab touch-none"
          style={{ width: view.w, height: view.h, imageRendering: "pixelated" }}
          aria-hidden
        />
        <div className="pointer-events-none absolute inset-0">
          {placed.map((p) => {
            const actor = actorsRef.current.get(p.id);
            const clickable = !!p.taskId;
            return (
              <button
                key={p.id}
                type="button"
                ref={(el) => {
                  if (el) bubbleRefs.current.set(p.id, el);
                  else bubbleRefs.current.delete(p.id);
                }}
                disabled={!clickable}
                onClick={() => p.taskId && onOpenTask(p.taskId)}
                className={cn(
                  "pointer-events-auto absolute left-0 top-0 flex items-center gap-1.5 whitespace-nowrap rounded-row px-2 py-0.5 text-micro font-bold opacity-0 shadow-press disabled:cursor-default",
                  bubbleTone(p, actor)
                )}
                title={clickable ? `打开 ${p.taskCode}` : undefined}
              >
                {bubbleText(p, actor, nicknames, phrases)}
                {p.taskCode && <span className="font-mono font-normal opacity-80">{p.taskCode}</span>}
              </button>
            );
          })}
        </div>
        {picked && (
          <div className="pointer-events-none absolute inset-0">
            <RobotCard
              key={picked.id}
              ref={cardRef}
              member={picked.id}
              name={name(picked.id, nicknames)}
              doing={doingText(picked, actorsRef.current.get(picked.id))}
              phrase={phrases.get(picked.id)}
              color={profiles.data?.find((p) => p.agent === picked.id)?.color}
              task={tasks.find((t) => t.id === picked.taskId)}
              onOpenTask={onOpenTask}
              onNewTask={projectId ? () => (setSelected(null), openNewTask(projectId)) : undefined}
              onClose={() => setSelected(null)}
            />
          </div>
        )}
        <div className="absolute right-3 top-3 flex items-center gap-2">
          {choice === "auto" && <SkyChip label={sky.label} />}
          <ThemeSwitch value={choice} onChange={setChoice} />
        </div>
        <ZoomControls pct={camera.zoomPct} onIn={() => camera.zoomBy(1.25)} onOut={() => camera.zoomBy(0.8)} onReset={camera.resetView} className="absolute bottom-3 right-3" />
      </div>
      {overflow.length > 0 && (
        <p className="text-meta text-t2">
          工位坐满了，还有 {overflow.length} 位在排队：{overflow.map((p) => `${name(p.id, nicknames)}（${p.taskCode ?? "待命"}）`).join("、")}
        </p>
      )}
      <ul className="sr-only">
        {placements.map((p) => (
          <li key={p.id}>
            {name(p.id, nicknames)}：{p.spot ? spotById(p.spot)?.label : "没有空位"}，{p.activity === "idle" ? "空闲" : VERB[p.activity]}
            {p.taskCode ? `，${p.taskCode}` : ""}
          </li>
        ))}
      </ul>
    </section>
  );
}
