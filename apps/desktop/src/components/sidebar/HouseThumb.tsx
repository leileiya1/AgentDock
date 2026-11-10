import { useEffect, useRef } from "react";
import daySpritesUrl from "@/assets/town/town-day-sprites.png";
import type { ProjectKind, Tier } from "@/lib/studio/buildings";
import { parseHex } from "@/lib/studio/paint";
import { TOWN, buildingSprite, paintSprite } from "@/lib/studio/townAtlas";
import { tinted } from "@/components/studio/paintCache";
import { cn } from "@/lib/utils";

let sprites: Promise<HTMLImageElement> | null = null;
function loadSprites(): Promise<HTMLImageElement> {
  sprites ??= new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = daySpritesUrl;
  });
  return sprites;
}

interface Props {
  kind: ProjectKind;
  tier: Tier;
  lit: boolean;
  roof?: string | null;
  size?: number;
  className?: string;
}

/** 侧栏里的小房子：就是小镇里这个项目的那栋楼（造型、规模、屋顶颜色一致），缩小画。 */
export function HouseThumb({ kind, tier, lit, roof, size = 32, className }: Props) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    let alive = true;
    loadSprites()
      .then((img) => {
        const canvas = ref.current;
        const ctx = canvas?.getContext("2d");
        if (!alive || !canvas || !ctx) return;
        const dpr = Math.max(1, window.devicePixelRatio || 1);
        const px = Math.round(size * dpr);
        canvas.width = canvas.height = px;
        ctx.clearRect(0, 0, px, px);
        const theme = TOWN.day;
        const b = buildingSprite(theme, kind, tier, lit);
        const scale = Math.min(px / b.w, px / b.h);
        const w = b.w * scale, h = b.h * scale;
        const x = (px - w) / 2, y = px - h;
        ctx.imageSmoothingEnabled = scale < 1;
        ctx.imageSmoothingQuality = "high";
        ctx.drawImage(img, b.rect[0], b.rect[1], b.w, b.h, x, y, w, h);
        const color = parseHex(roof);
        const paint = color && paintSprite(theme, kind, tier, lit);
        if (color && paint) {
          const layer = tinted(img, paint.rect, color, TOWN.paintGrey, "town-day");
          if (layer) ctx.drawImage(layer, x + (paint.dx - b.dx) * scale, y + (paint.dy - b.dy) * scale, paint.w * scale, paint.h * scale);
        }
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [kind, tier, lit, roof, size]);
  return <canvas ref={ref} aria-hidden className={cn("shrink-0", className)} style={{ width: size, height: size }} />;
}
