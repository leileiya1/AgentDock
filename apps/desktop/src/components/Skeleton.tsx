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

export function SkeletonRows({ rows = 4, gap = 10 }: { rows?: number; gap?: number }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap }}>
      {Array.from({ length: rows }).map((_, i) => (
        <Skeleton key={i} height={36} />
      ))}
    </div>
  );
}
