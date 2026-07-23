import { useSyncExternalStore } from "react";

/**
 * 三档布局 (05 §8):
 *   wide    ≥1440px — 执行树 + run 列表 + 内容 三栏并列
 *   medium  1024–1439px — 三栏放不下，run 列表改为可折叠
 *   compact <1024px — 导航（执行树）进抽屉，内容占满
 *
 * 这同时也是 200% 缩放的适配路径：1440px 屏幕在 200% 下等于 720 CSS px，
 * 会落进 compact，侧栏让位而不是把内容裁掉。
 */
export type Layout = "compact" | "medium" | "wide";

const QUERIES: Array<[Layout, string]> = [
  ["wide", "(min-width: 1440px)"],
  ["medium", "(min-width: 1024px)"],
];

function current(): Layout {
  if (typeof window === "undefined" || !window.matchMedia) return "wide";
  for (const [layout, query] of QUERIES) {
    if (window.matchMedia(query).matches) return layout;
  }
  return "compact";
}

function subscribe(onChange: () => void): () => void {
  if (typeof window === "undefined" || !window.matchMedia) return () => {};
  const lists = QUERIES.map(([, query]) => window.matchMedia(query));
  for (const list of lists) list.addEventListener("change", onChange);
  return () => {
    for (const list of lists) list.removeEventListener("change", onChange);
  };
}

export function useLayout(): Layout {
  // useSyncExternalStore keeps this correct across concurrent renders and avoids
  // the resize-listener + setState pattern re-rendering on every pixel.
  return useSyncExternalStore(subscribe, current, () => "wide" as Layout);
}
