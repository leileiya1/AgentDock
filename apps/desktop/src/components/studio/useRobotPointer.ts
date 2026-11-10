import { useEffect, useRef, type MutableRefObject, type RefObject } from "react";
import { toScene, type Camera } from "@/lib/studio/camera";
import { pickRobot, type RobotBox } from "@/lib/studio/robotHit";

interface Options {
  canvasRef: RefObject<HTMLCanvasElement>;
  camRef: MutableRefObject<Camera>;
  view: { w: number; h: number };
  /** 每帧由画布更新的机器人命中框。 */
  boxesRef: MutableRefObject<RobotBox[]>;
  /** 点中机器人给 id，点空地给 null。 */
  onPick: (id: string | null) => void;
}

/** 小屋里点机器人：拖动超过 5px 算平移；悬停在机器人上换手形。 */
export function useRobotPointer({ canvasRef, camRef, view, boxesRef, onPick }: Options) {
  const latest = useRef({ view, onPick });
  latest.current = { view, onPick };

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    let down: { x: number; y: number } | null = null;
    const at = (e: PointerEvent) => {
      const r = canvas.getBoundingClientRect();
      const p = toScene(camRef.current, latest.current.view, e.clientX - r.left, e.clientY - r.top);
      return pickRobot(boxesRef.current, p.x, p.y);
    };
    const onDown = (e: PointerEvent) => {
      down = { x: e.clientX, y: e.clientY };
    };
    const onUp = (e: PointerEvent) => {
      const start = down;
      down = null;
      if (!start || e.button !== 0 || Math.hypot(e.clientX - start.x, e.clientY - start.y) > 5) return;
      latest.current.onPick(at(e));
    };
    const onMove = (e: PointerEvent) => {
      if (!down) canvas.style.cursor = at(e) ? "pointer" : "";
    };
    canvas.addEventListener("pointerdown", onDown);
    canvas.addEventListener("pointerup", onUp);
    canvas.addEventListener("pointermove", onMove);
    return () => {
      canvas.removeEventListener("pointerdown", onDown);
      canvas.removeEventListener("pointerup", onUp);
      canvas.removeEventListener("pointermove", onMove);
    };
  }, [canvasRef, camRef, boxesRef]);
}
