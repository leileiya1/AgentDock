import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import type { Project, ProjectAppearance, ProjectSummary } from "@/generated/bindings";
import dayTilesUrl from "@/assets/town/town-day-tiles.png";
import nightTilesUrl from "@/assets/town/town-night-tiles.png";
import daySpritesUrl from "@/assets/town/town-day-sprites.png";
import nightSpritesUrl from "@/assets/town/town-night-sprites.png";
import dayRobotsUrl from "@/assets/studio/studio-day-atlas.png";
import nightRobotsUrl from "@/assets/studio/studio-night-atlas.png";
import { buildingFor, lookFromAppearance } from "@/lib/studio/buildings";
import { townAgents } from "@/lib/studio/projectSummary";
import { FACILITY_LABEL, layoutCity } from "@/lib/studio/cityLayout";
import { MANIFEST } from "@/lib/studio/atlas";
import { TOWN, blockOrigin, townGeometry } from "@/lib/studio/townAtlas";
import { projectLook } from "@/lib/studio/townResidents";
import { justBuilt } from "@/lib/studio/townEvents";
import { useCelebrateStore } from "@/stores/celebrateStore";
import { toScreen } from "@/lib/studio/camera";
import { useCanvasCamera } from "@/components/studio/useCanvasCamera";
import { ThemeSwitch, ZoomControls, type ThemeChoice } from "@/components/studio/SceneControls";
import { useSky } from "@/hooks/useSky";
import { SkyChip } from "@/components/studio/skyTint";
import { drawTint } from "@/components/studio/skyTint";
import { drawTown, type TownImages, type TownProject } from "@/components/studio/town/drawTown";
import { TownSign } from "@/components/studio/town/TownSign";
import { cn } from "@/lib/utils";
import { hitTown } from "@/lib/studio/townHit";
import { useTownPointer } from "@/components/studio/town/useTownPointer";
import { useAgentProfiles } from "@/hooks/useStudio";
import { robotLooks } from "@/components/studio/drawRobot";

export interface TownViewHandle {
  /** 镜头飞到某个项目的房子。 */
  focusProject: (projectId: string) => void;
}

function viewportFor(width: number) {
  return { w: Math.max(320, width), h: Math.round(Math.min(720, Math.max(420, width * 0.6))) };
}

function load(src: string): Promise<HTMLImageElement> {
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
 * 项目小镇：一个项目一栋房子，造型由项目类型决定，大小由规模决定；外观状态和院子里机器人的人数
 * 只来自后端项目汇总。拖动看别的项目，点名牌进入项目。
 */
interface TownViewProps {
  projects: Project[];
  summaries: ProjectSummary[];
  /** 用户保存的外观（类型、屋顶、门牌、摆件）；没有就按项目名猜。 */
  appearance?: ProjectAppearance[];
  onOpen: (projectId: string) => void;
  /** 点门牌上的小刷子：装修这个项目。 */
  onDecorate?: (projectId: string) => void;
}

export const TownView = forwardRef<TownViewHandle, TownViewProps>(
  function TownView({ projects, summaries, appearance = [], onOpen, onDecorate }, ref) {
    const wrapRef = useRef<HTMLDivElement>(null);
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const signRefs = useRef(new Map<string, HTMLElement>());
    const placeRefs = useRef(new Map<string, HTMLElement>());
    const [choice, setChoice] = useState<ThemeChoice>("auto");
    const [images, setImages] = useState<Record<"day" | "night", TownImages> | null>(null);
    const reduce = useMemo(prefersReducedMotion, []);
    const sky = useSky(choice);
    const theme = sky.theme;
    const skyRef = useRef(sky);
    skyRef.current = sky;

    const city = useMemo(() => layoutCity(projects.map((p) => p.id)), [projects]);
    const geometry = useMemo(() => townGeometry(city), [city]);
    const scene = useMemo(() => ({ w: geometry.w, h: geometry.h }), [geometry]);
    const hallCenter = useMemo(() => {
      const hall = city.cells.find((c) => c.type === "facility" && c.facility === "hall") ?? city.cells[0];
      const [x, y] = blockOrigin(geometry, hall);
      return { x, y: y + TOWN.block - 20 };
    }, [city, geometry]);

    const townProjects = useMemo(() => {
      const bySummary = new Map(summaries.map((s) => [s.projectId, s]));
      const byLook = new Map(appearance.map((a) => [a.projectId, lookFromAppearance(a)]));
      const now = Date.now();
      return new Map<string, TownProject>(
        projects.map((p) => {
          const s = bySummary.get(p.id);
          const b = buildingFor(p.name, byLook.get(p.id), { agents: townAgents(s).length, tasks: s?.taskCount ?? 0 });
          return [p.id, { id: p.id, name: p.name, kind: b.kind, tier: b.tier, look: projectLook(s, now), house: byLook.get(p.id), summary: s }];
        })
      );
    }, [projects, summaries, appearance]);
    const profiles = useAgentProfiles();
    const looks = useMemo(() => robotLooks(profiles.data), [profiles.data]);
    const looksRef = useRef(looks);
    looksRef.current = looks;
    const townRef = useRef(townProjects);
    townRef.current = townProjects;

    // 工地派了第一个任务 → 新房落成，放烟花
    const sitesRef = useRef<Set<string> | null>(null);
    useEffect(() => {
      if (!summaries.length) return;
      const sites = new Set([...townProjects.values()].filter((p) => p.look.construction).map((p) => p.id));
      for (const id of justBuilt(sitesRef.current, sites, [...townProjects.keys()])) {
        useCelebrateStore.getState().celebrate({ text: `「${townProjects.get(id)?.name}」新房落成！`, projectId: id });
      }
      sitesRef.current = sites;
    }, [townProjects, summaries.length]);

    const camera = useCanvasCamera(wrapRef, canvasRef, scene, viewportFor, { center: hallCenter, zoom: 1.15 }, reduce);
    const { view, camRef } = camera;

    const [hovered, setHovered] = useState<string | null>(null);
    useTownPointer({
      canvasRef,
      camRef,
      view,
      hit: (x, y) => hitTown(city, geometry, TOWN[theme], townRef.current, x, y),
      onPick: (h) => {
        if (h.type === "project") onOpen(h.id);
        else {
          const [x, y] = blockOrigin(geometry, h.cell);
          camera.flyTo(x, y + TOWN.block - 20, 1.6);
        }
      },
      onHover: setHovered,
    });

    useImperativeHandle(ref, () => ({
      focusProject: (projectId) => {
        const cell = city.cells.find((c) => c.type === "project" && c.projectId === projectId);
        if (!cell) return;
        const [x, y] = blockOrigin(geometry, cell);
        camera.flyTo(x, y + TOWN.block - 30, 1.8);
      },
    }));

    useEffect(() => {
      let alive = true;
      Promise.all([dayTilesUrl, daySpritesUrl, dayRobotsUrl, nightTilesUrl, nightSpritesUrl, nightRobotsUrl].map(load))
        .then(([dt, ds, dr, nt, ns, nr]) => {
          if (alive) setImages({ day: { tiles: dt, sprites: ds, robots: dr }, night: { tiles: nt, sprites: ns, robots: nr } });
        })
        .catch(() => undefined);
      return () => {
        alive = false;
      };
    }, []);

    useEffect(() => {
      const canvas = canvasRef.current;
      const ctx = canvas?.getContext("2d");
      if (!canvas || !ctx || !images) return;
      const dpr = Math.max(1, window.devicePixelRatio || 1);
      canvas.width = Math.round(view.w * dpr);
      canvas.height = Math.round(view.h * dpr);
      let raf = 0, last = performance.now(), time = 0;

      const place = (el: HTMLElement | undefined, x: number, y: number, visible: boolean) => {
        if (!el) return;
        const p = toScreen(camRef.current, view, x, y);
        el.style.transform = `translate(${Math.round(p.x)}px, ${Math.round(p.y)}px) translate(-50%, -50%)`;
        el.style.opacity = visible ? "1" : "0";
      };

      const frame = () => {
        const cam = camRef.current;
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.clearRect(0, 0, view.w, view.h);
        ctx.setTransform(dpr * cam.z, 0, 0, dpr * cam.z, Math.round(dpr * (view.w / 2 - cam.x * cam.z)), Math.round(dpr * (view.h / 2 - cam.y * cam.z)));
        ctx.imageSmoothingEnabled = false;
        const { signs, places } = drawTown({
          ctx, geometry, city, projects: townRef.current, theme: TOWN[theme], robots: MANIFEST[theme], images: images[theme], time, reduce,
          themeKey: theme, looks: looksRef.current, sleepy: skyRef.current.sleepy,
        });
        drawTint(ctx, dpr, view, skyRef.current.tint);
        for (const s of signs) place(signRefs.current.get(s.key), s.x, s.y, true);
        for (const p of places) place(placeRefs.current.get(p.key), p.x, p.y, cam.z >= 0.9);
      };
      const loop = (now: number) => {
        raf = requestAnimationFrame(loop);
        if (document.hidden) {
          last = now;
          return;
        }
        const dt = Math.max(0, Math.min(0.1, (now - last) / 1000));
        last = now;
        time += reduce ? 0 : dt;
        camera.step(dt);
        frame();
      };
      frame();
      raf = requestAnimationFrame(loop);
      return () => cancelAnimationFrame(raf);
      // camera.step only reads refs
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [images, theme, view, geometry, city, reduce]);

    const placeKeys = useMemo(() => [...new Set(city.cells.filter((c) => c.type !== "project").map((c) => (c.type === "facility" ? c.facility : c.filler)))], [city]);

    return (
      <div
        ref={wrapRef}
        tabIndex={0}
        onKeyDown={camera.onKeyDown}
        aria-label="项目小镇：拖动查看其他项目，滚轮或双指缩放，点房子或名牌进入项目"
        className={cn("relative overflow-hidden rounded-card border border-line outline-none focus-visible:ring-2 focus-visible:ring-focus", theme === "night" ? "bg-studio-night" : "bg-studio-day")}
        style={{ height: view.h }}
      >
        <canvas ref={canvasRef} className="block cursor-grab touch-none" style={{ width: view.w, height: view.h, imageRendering: "pixelated" }} aria-hidden />
        <div className="pointer-events-none absolute inset-0">
          {placeKeys.map((k) => (
            <span
              key={k}
              ref={(el) => {
                if (el) placeRefs.current.set(k, el);
                else placeRefs.current.delete(k);
              }}
              className="absolute left-0 top-0 whitespace-nowrap rounded-row bg-raised/85 px-2 py-0.5 text-micro font-bold text-t2 opacity-0 ring-1 ring-line transition-opacity"
            >
              {FACILITY_LABEL[k as keyof typeof FACILITY_LABEL]}
            </span>
          ))}
          {[...townProjects.values()].map((p) => (
            <TownSign
              key={p.id}
              project={p}
              ref={(el) => {
                if (el) signRefs.current.set(p.id, el);
                else signRefs.current.delete(p.id);
              }}
              highlighted={hovered === p.id}
              onOpen={() => onOpen(p.id)}
              onDecorate={() => onDecorate?.(p.id)}
            />
          ))}
        </div>
        <div className="absolute right-3 top-3 flex items-center gap-2">
          {choice === "auto" && <SkyChip label={sky.label} />}
          <ThemeSwitch value={choice} onChange={setChoice} />
        </div>
        <ZoomControls pct={camera.zoomPct} onIn={() => camera.zoomBy(1.25)} onOut={() => camera.zoomBy(0.8)} onReset={camera.resetView} className="absolute bottom-3 right-3" />
      </div>
    );
  }
);
