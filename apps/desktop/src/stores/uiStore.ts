import { create } from "zustand";

export type DetailTab = "overview" | "logs" | "diff" | "review" | "governance";

interface UiState {
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;

  /** per-task active detail tab */
  activeTab: Record<string, DetailTab>;
  setActiveTab: (taskId: string, tab: DetailTab) => void;

  /** per-task selected revision for the timeline / context switch */
  selectedRevision: Record<string, number>;
  setSelectedRevision: (taskId: string, rev: number) => void;

  /** per-task selected run in the logs tab */
  selectedRun: Record<string, string>;
  setSelectedRun: (taskId: string, runId: string) => void;

  /** diff tab jump target set from an issue card (file + line) */
  diffJump: { taskId: string; file: string; line: number | null } | null;
  requestDiffJump: (taskId: string, file: string, line: number | null) => void;
  clearDiffJump: () => void;

  /** new-task dialog (global, so Cmd/Ctrl+N can open it from any page) */
  newTaskProjectId: string | null;
  openNewTask: (projectId: string) => void;
  closeNewTask: () => void;
}

export const useUiStore = create<UiState>((set) => ({
  sidebarCollapsed: false,
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),

  activeTab: {},
  setActiveTab: (taskId, tab) =>
    set((s) => ({ activeTab: { ...s.activeTab, [taskId]: tab } })),

  selectedRevision: {},
  setSelectedRevision: (taskId, rev) =>
    set((s) => ({ selectedRevision: { ...s.selectedRevision, [taskId]: rev } })),

  selectedRun: {},
  setSelectedRun: (taskId, runId) =>
    set((s) => ({ selectedRun: { ...s.selectedRun, [taskId]: runId } })),

  diffJump: null,
  requestDiffJump: (taskId, file, line) =>
    set((s) => ({
      diffJump: { taskId, file, line },
      activeTab: { ...s.activeTab, [taskId]: "diff" as DetailTab },
    })),
  clearDiffJump: () => set({ diffJump: null }),

  newTaskProjectId: null,
  openNewTask: (projectId) => set({ newTaskProjectId: projectId }),
  closeNewTask: () => set({ newTaskProjectId: null }),
}));
