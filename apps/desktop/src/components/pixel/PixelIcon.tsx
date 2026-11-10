import { cn } from "@/lib/utils";

/**
 * 8×8 pixel icons that share the squad's look. They inherit `currentColor`, so
 * tone comes from the surrounding text utility. Icon-only usages must still get
 * an accessible name from the parent control.
 */
const PATHS = {
  branch: "M1 0h2v2h-2z M1 2h1v4h-1z M1 6h2v2h-2z M5 1h2v2h-2z M5 3h1v1h-1z M3 4h2v1h-2z",
  clock: "M2 0h4v1h-4z M1 1h1v1h-1z M6 1h1v1h-1z M0 2h1v4h-1z M7 2h1v4h-1z M1 6h1v1h-1z M6 6h1v1h-1z M2 7h4v1h-4z M3 2h1v3h-1z M4 4h2v1h-2z",
  hourglass: "M0 0h8v1h-8z M1 1h6v1h-6z M2 2h4v1h-4z M3 3h2v2h-2z M2 5h4v1h-4z M1 6h6v1h-6z M0 7h8v1h-8z",
  shield: "M1 0h6v1h-6z M0 1h1v4h-1z M7 1h1v4h-1z M1 5h1v1h-1z M6 5h1v1h-1z M2 6h1v1h-1z M5 6h1v1h-1z M3 7h2v1h-2z M3 2h2v3h-2z",
  flask: "M2 0h4v1h-4z M3 1h2v2h-2z M2 3h1v1h-1z M5 3h1v1h-1z M1 4h1v1h-1z M6 4h1v1h-1z M0 5h8v3h-8z",
  eye: "M2 1h4v1h-4z M1 2h1v1h-1z M6 2h1v1h-1z M0 3h1v2h-1z M7 3h1v2h-1z M1 5h1v1h-1z M6 5h1v1h-1z M2 6h4v1h-4z M3 3h2v2h-2z",
  bolt: "M4 0h3v1h-3z M3 1h3v1h-3z M2 2h3v1h-3z M1 3h6v1h-6z M3 4h3v1h-3z M2 5h3v1h-3z M1 6h3v1h-3z M1 7h1v1h-1z",
  key: "M0 1h3v1h-3z M0 2h1v2h-1z M2 2h1v2h-1z M0 4h3v1h-3z M3 2h5v1h-5z M5 3h1v2h-1z M7 3h1v1h-1z",
  alert: "M3 0h2v5h-2z M3 6h2v2h-2z",
  check: "M6 1h2v1h-2z M5 2h2v1h-2z M4 3h2v1h-2z M3 4h2v1h-2z M2 5h2v1h-2z M0 3h2v1h-2z M1 4h2v1h-2z",
  cross: "M0 0h2v1h-2z M1 1h2v1h-2z M2 2h2v1h-2z M3 3h2v2h-2z M4 2h2v1h-2z M5 1h2v1h-2z M6 0h2v1h-2z M2 5h2v1h-2z M1 6h2v1h-2z M0 7h2v1h-2z M4 5h2v1h-2z M5 6h2v1h-2z M6 7h2v1h-2z",
  pause: "M1 1h2v6h-2z M5 1h2v6h-2z",
  stop: "M1 1h6v6h-6z",
  play: "M1 0h2v8h-2z M3 1h1v6h-1z M4 2h1v4h-1z M5 3h1v2h-1z",
  plus: "M3 0h2v8h-2z M0 3h8v2h-8z",
  search: "M1 0h4v1h-4z M0 1h1v4h-1z M5 1h1v4h-1z M1 5h4v1h-4z M5 5h1v1h-1z M6 6h2v2h-2z",
  gear: "M2 1h4v1h1v4h-1v1h-4v-1h-1v-4h1z M3 3v2h2v-2z M3 0h2v1h-2z M3 7h2v1h-2z M0 3h1v2h-1z M7 3h1v2h-1z",
  file: "M1 0h5v1h1v7h-6z M2 1v6h4v-5h-1v-1z M3 3h2v1h-2z M3 5h2v1h-2z",
  list: "M0 1h2v2h-2z M3 1h5v2h-5z M0 5h2v2h-2z M3 5h5v2h-5z",
  squad: "M3 0h2v1h-2z M1 1h6v5h-6z M2 3h1v1h-1z M5 3h1v1h-1z M2 6h4v2h-4z",
  coffee: "M1 3h5v4h-5z M6 4h2v2h-2z M2 0h1v2h-1z M4 0h1v2h-1z",
  globe: "M2 0h4v1h-4z M1 1h1v1h-1z M6 1h1v1h-1z M0 2h1v4h-1z M7 2h1v4h-1z M1 6h1v1h-1z M6 6h1v1h-1z M2 7h4v1h-4z M3 1h2v6h-2z M1 3h6v2h-6z",
  package: "M0 1h8v2h-8z M1 3h6v5h-6z M3 4v1h2v-1z",
  lock: "M2 0h4v1h-4z M1 1h1v3h-1z M6 1h1v3h-1z M0 4h8v4h-8z",
  swap: "M1 1h5v1h-5z M5 0h1v1h-1z M5 2h1v1h-1z M2 6h5v1h-5z M2 5h1v1h-1z M2 7h1v1h-1z",
  download: "M3 0h2v4h-2z M1 3h6v1h-6z M2 4h4v1h-4z M3 5h2v1h-2z M0 7h8v1h-8z",
  chevronLeft: "M4 1h2v1h-2z M3 2h2v1h-2z M2 3h2v2h-2z M3 5h2v1h-2z M4 6h2v1h-2z",
  chevronRight: "M2 1h2v1h-2z M3 2h2v1h-2z M4 3h2v2h-2z M3 5h2v1h-2z M2 6h2v1h-2z",
} as const;

export type PixelIconName = keyof typeof PATHS;

const EVEN_ODD = new Set<PixelIconName>(["gear", "file", "squad", "package"]);

export function PixelIcon({
  name,
  size = 12,
  className,
  label,
}: {
  name: PixelIconName;
  size?: number;
  className?: string;
  /** Provide only when the icon is the sole carrier of meaning. */
  label?: string;
}) {
  return (
    <svg
      viewBox="0 0 8 8"
      width={size}
      height={size}
      className={cn("pixelated inline-block shrink-0", className)}
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      <path fill="currentColor" fillRule={EVEN_ODD.has(name) ? "evenodd" : undefined} d={PATHS[name]} />
    </svg>
  );
}
