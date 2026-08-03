import { useEffect, useState } from "react";
import { Skeleton as UiSkeleton } from "@/components/ui/skeleton";

interface Props {
  height?: number | string;
  width?: number | string;
  radius?: number;
}

/** Detection/loading uses skeletons, never a spinner overlay (02 §4.1). */
export function Skeleton({ height = 16, width = "100%", radius = 6 }: Props) {
  return <UiSkeleton style={{ height, width, borderRadius: radius }} />;
}

export function SkeletonRows({ rows = 4, gap = 10, delayMs = 400 }: { rows?: number; gap?: number; delayMs?: number }) {
  const [visible, setVisible] = useState(delayMs === 0);

  useEffect(() => {
    if (delayMs === 0) return;
    const timer = window.setTimeout(() => setVisible(true), delayMs);
    return () => window.clearTimeout(timer);
  }, [delayMs]);

  if (!visible) {
    return <div aria-hidden="true" style={{ minHeight: rows * 36 + Math.max(0, rows - 1) * gap }} />;
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap }}>
      {Array.from({ length: rows }).map((_, i) => (
        <Skeleton key={i} height={36} />
      ))}
    </div>
  );
}
