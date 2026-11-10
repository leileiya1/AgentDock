import { useEffect, useRef, type MutableRefObject, type RefObject } from "react";
import { toScene, type Camera } from "@/lib/studio/camera";
import type { TownHit } from "@/lib/studio/townHit";

interface Options {
  canvasRef: RefObject<HTMLCanvasElement>;
  camRef: MutableRefObject<Camera>;
  view: { w: number; h: number };
  /** 场景像素 → 命中结果。 */
  hit: (x: number, y: number) => TownHit;
  onPick: (hit: NonNullable<TownHit>) => void;
  onHover: (projectId: string | null) => void;
}

/**
 * 点房子进项目、点公共设施飞过去；悬停在房子上时换成手形并高亮门牌。
 * 拖动超过几像素算平移，不算点击（镜头的拖动由 useCanvasCamera 处理）。
 */
export function useTownPointer({ canvasRef, camRef, view, hit, onPick, onHover }: Options) {
  const latest = useRef({ hit, onPick, onHover, view });
  latest.current = { hit, onPick, onHover, view };

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    let down: { x: number; y: number } | null = null;
    let hovered: string | null = null;
    const at = (e: MouseEvent) => {
      const r = canvas.getBoundingClientRect();
      const p = toScene(camRef.current, latest.current.view, e.clientX - r.left, e.clientY - r.top);
      return latest.current.hit(p.x, p.y);
    };
    const onDown = (e: PointerEvent) => {
      down = { x: e.clientX, y: e.clientY };
    };
    const onUp = (e: PointerEvent) => {
      const start = down;
      down = null;
      if (!start || Math.hypot(e.clientX - start.x, e.clientY - start.y) > 5 || e.button !== 0) return;
      const h = at(e);
      if (h) latest.current.onPick(h);
    };
    const onMove = (e: PointerEvent) => {
      if (down) return; // dragging
      const h = at(e);
      const id = h?.type === "project" ? h.id : null;
      canvas.style.cursor = id ? "pointer" : "";
      if (id !== hovered) {
        hovered = id;
        latest.current.onHover(id);
      }
    };
    const onLeave = () => {
      if (hovered) latest.current.onHover((hovered = null));
    };
    canvas.addEventListener("pointerdown", onDown);
    canvas.addEventListener("pointerup", onUp);
    canvas.addEventListener("pointermove", onMove);
    canvas.addEventListener("pointerleave", onLeave);
    return () => {
      canvas.removeEventListener("pointerdown", onDown);
      canvas.removeEventListener("pointerup", onUp);
      canvas.removeEventListener("pointermove", onMove);
      canvas.removeEventListener("pointerleave", onLeave);
    };
  }, [canvasRef, camRef]);
}
