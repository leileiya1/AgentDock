import { create } from "zustand";
import type { AgentKind } from "@/generated/bindings";

export interface Celebration {
  id: number;
  at: number;
  /** 横幅文字。 */
  text: string;
  projectId?: string;
  /** 合并任务的开发者，小屋里会蹦一蹦。 */
  agent?: AgentKind;
}

interface CelebrateState {
  queue: Celebration[];
  celebrate: (c: Omit<Celebration, "id" | "at">) => void;
  done: (id: number) => void;
}

let seq = 0;

/** 庆祝事件：合并成功（或 ⌘K 彩蛋）时推进来，烟花层播完就移走。 */
export const useCelebrateStore = create<CelebrateState>((set) => ({
  queue: [],
  celebrate: (c) => set((s) => ({ queue: [...s.queue.slice(-3), { ...c, id: ++seq, at: Date.now() }] })),
  done: (id) => set((s) => ({ queue: s.queue.filter((c) => c.id !== id) })),
}));
