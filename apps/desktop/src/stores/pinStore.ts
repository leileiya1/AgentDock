import { create } from "zustand";

export type ProjectSort = "default" | "waiting" | "recent";

const PIN_KEY = "agentflow.pinnedProjects";
const SORT_KEY = "agentflow.projectSort";

function read<T>(key: string, fallback: T, valid: (v: unknown) => v is T): T {
  try {
    const raw = localStorage.getItem(key);
    const v = raw ? JSON.parse(raw) : null;
    return valid(v) ? v : fallback;
  } catch {
    return fallback;
  }
}
function write(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* per-viewer convenience only */
  }
}
const isIds = (v: unknown): v is string[] => Array.isArray(v) && v.every((x) => typeof x === "string");
const isSort = (v: unknown): v is ProjectSort => v === "default" || v === "waiting" || v === "recent";

interface PinState {
  /** 置顶的项目，按置顶先后排。 */
  pinned: string[];
  togglePin: (projectId: string) => void;
  sort: ProjectSort;
  cycleSort: () => void;
}

const NEXT: Record<ProjectSort, ProjectSort> = { default: "waiting", waiting: "recent", recent: "default" };

/** 侧栏的「常去」和排序方式：只是这台电脑上的偏好，存 localStorage。 */
export const usePinStore = create<PinState>((set) => ({
  pinned: read(PIN_KEY, [], isIds),
  togglePin: (id) =>
    set((s) => {
      const pinned = s.pinned.includes(id) ? s.pinned.filter((x) => x !== id) : [...s.pinned, id];
      write(PIN_KEY, pinned);
      return { pinned };
    }),
  sort: read(SORT_KEY, "default", isSort),
  cycleSort: () =>
    set((s) => {
      const sort = NEXT[s.sort];
      write(SORT_KEY, sort);
      return { sort };
    }),
}));
