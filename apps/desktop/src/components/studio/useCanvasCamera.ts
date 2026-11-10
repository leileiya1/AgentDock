import { useEffect, useRef, useState, type KeyboardEvent, type MutableRefObject, type RefObject } from "react";
import { approach, clampCamera, homeCamera, panBy, zoomAt, type Camera, type Scene } from "@/lib/studio/camera";

export interface Viewport {
  w: number;
  h: number;
}

export interface CanvasCamera {
  view: Viewport;
  /** 当前镜头（每帧被 step 推向 target），绘制时读它。 */
  camRef: MutableRefObject<Camera>;
  zoomPct: number;
  zoomBy: (factor: number, at?: { x: number; y: number }) => void;
  resetView: () => void;
  /** 平滑飞到场景里某一点（原尺寸像素坐标），可选缩放倍数（相对默认镜头）。 */
  flyTo: (x: number, y: number, zoomMul?: number) => void;
  /** 每帧调用：镜头平滑靠近目标。 */
  step: (dt: number) => void;
  onKeyDown: (e: KeyboardEvent) => void;
}

/**
 * 像素场景通用镜头：容器宽度决定画面大小，滚轮 / 双指缩放、拖动平移、双击放大，
 * 键盘加减号缩放、方向键平移、0 复位。小屋和小镇共用。
 */
export function useCanvasCamera(
  wrapRef: RefObject<HTMLElement>,
  canvasRef: RefObject<HTMLCanvasElement>,
  scene: Scene,
  viewportFor: (width: number) => Viewport,
  home: { center?: { x: number; y: number }; zoom?: number },
  reduce: boolean
): CanvasCamera {
  const [view, setView] = useState<Viewport>(() => viewportFor(960));
  const viewRef = useRef(view);
  viewRef.current = view;
  const sceneRef = useRef(scene);
  sceneRef.current = scene;
  const homeRef = useRef(home);
  homeRef.current = home;
  const camRef = useRef<Camera>(homeCamera(view, scene, home));
  const targetRef = useRef<Camera>(camRef.current);
  const [zoomPct, setZoomPct] = useState(100);

  const setTarget = (c: Camera, immediate = false) => {
    targetRef.current = c;
    if (immediate || reduce) camRef.current = c;
    setZoomPct(Math.round((c.z / homeCamera(viewRef.current, sceneRef.current, homeRef.current).z) * 100));
  };
  const zoomBy = (factor: number, at?: { x: number; y: number }) =>
    setTarget(zoomAt(targetRef.current, factor, at ?? { x: viewRef.current.w / 2, y: viewRef.current.h / 2 }, viewRef.current, sceneRef.current));
  const resetView = () => setTarget(homeCamera(viewRef.current, sceneRef.current, homeRef.current));
  const flyTo = (x: number, y: number, zoomMul = 1.6) => {
    const base = homeCamera(viewRef.current, sceneRef.current, homeRef.current);
    setTarget(clampCamera({ x, y, z: base.z * zoomMul }, viewRef.current, sceneRef.current));
  };
  const step = (dt: number) => {
    camRef.current = reduce ? targetRef.current : approach(camRef.current, targetRef.current, dt);
  };

  // viewport follows the container width; the first fit also sets the home camera
  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    let first = true;
    const fit = () => {
      const v = viewportFor(el.clientWidth);
      setView((old) => (old.w === v.w && old.h === v.h ? old : v));
      if (first) {
        camRef.current = targetRef.current = homeCamera(v, sceneRef.current, homeRef.current);
        first = false;
      } else {
        camRef.current = clampCamera(camRef.current, v, sceneRef.current);
        targetRef.current = clampCamera(targetRef.current, v, sceneRef.current);
      }
    };
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
    // viewportFor is a stable module-level function in callers
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wrapRef]);

  // scene size can change (a new project grows the town): keep the camera inside it
  useEffect(() => {
    camRef.current = clampCamera(camRef.current, viewRef.current, scene);
    targetRef.current = clampCamera(targetRef.current, viewRef.current, scene);
  }, [scene.w, scene.h]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const local = (e: { clientX: number; clientY: number }) => {
      const r = canvas.getBoundingClientRect();
      return { x: e.clientX - r.left, y: e.clientY - r.top };
    };
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const unit = e.deltaMode === 1 ? 16 : 1;
      setTarget(zoomAt(targetRef.current, Math.exp(-e.deltaY * unit * (e.ctrlKey ? 0.01 : 0.0015)), local(e), viewRef.current, sceneRef.current));
    };
    let drag: { x: number; y: number; id: number } | null = null;
    const onDown = (e: PointerEvent) => {
      drag = { ...local(e), id: e.pointerId };
      canvas.setPointerCapture(e.pointerId);
      canvas.style.cursor = "grabbing";
    };
    const onMove = (e: PointerEvent) => {
      if (!drag || drag.id !== e.pointerId) return;
      const p = local(e);
      setTarget(panBy(targetRef.current, p.x - drag.x, p.y - drag.y, viewRef.current, sceneRef.current), true);
      drag = { ...p, id: e.pointerId };
    };
    const onUp = (e: PointerEvent) => {
      if (drag?.id === e.pointerId) drag = null;
      canvas.style.cursor = "";
    };
    const onDbl = (e: MouseEvent) => zoomBy(1.8, local(e));
    canvas.addEventListener("wheel", onWheel, { passive: false });
    canvas.addEventListener("pointerdown", onDown);
    canvas.addEventListener("pointermove", onMove);
    canvas.addEventListener("pointerup", onUp);
    canvas.addEventListener("pointercancel", onUp);
    canvas.addEventListener("dblclick", onDbl);
    return () => {
      canvas.removeEventListener("wheel", onWheel);
      canvas.removeEventListener("pointerdown", onDown);
      canvas.removeEventListener("pointermove", onMove);
      canvas.removeEventListener("pointerup", onUp);
      canvas.removeEventListener("pointercancel", onUp);
      canvas.removeEventListener("dblclick", onDbl);
    };
    // handlers only read refs
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [canvasRef]);

  const onKeyDown = (e: KeyboardEvent) => {
    const v = viewRef.current, s = sceneRef.current, by = 60;
    if (e.key === "+" || e.key === "=") zoomBy(1.25);
    else if (e.key === "-" || e.key === "_") zoomBy(0.8);
    else if (e.key === "0") resetView();
    else if (e.key === "ArrowLeft") setTarget(panBy(targetRef.current, by, 0, v, s));
    else if (e.key === "ArrowRight") setTarget(panBy(targetRef.current, -by, 0, v, s));
    else if (e.key === "ArrowUp") setTarget(panBy(targetRef.current, 0, by, v, s));
    else if (e.key === "ArrowDown") setTarget(panBy(targetRef.current, 0, -by, v, s));
    else return;
    e.preventDefault();
  };

  return { view, camRef, zoomPct, zoomBy, resetView, flyTo, step, onKeyDown };
}
