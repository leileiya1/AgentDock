import { create } from "zustand";
import type { TreeSelection } from "@/components/execution/TreeNodes";

export type DetailTab = "overview" | "logs" | "diff" | "review" | "governance";
export type DensityMode = "comfortable" | "compact";

interface UiState {
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;

  /** High-information views may tighten rows without changing content or color. */
  densityMode: DensityMode;
  toggleDensity: () => void;

  /** per-task active detail tab */
  activeTab: Record<string, DetailTab>;
  setActiveTab: (taskId: string, tab: DetailTab) => void;

  /** per-task selected revision for the timeline / context switch */
  selectedRevision: Record<string, number>;
  setSelectedRevision: (taskId: string, rev: number) => void;

  /** per-task selected run in the logs tab */
  selectedRun: Record<string, string>;
  setSelectedRun: (taskId: string, runId: string) => void;

  /**
   * Execution-tree selection. One click must move the tree, the run list and the
   * content pane together — three panes disagreeing was P0-04.
   */
  treeSelection: Record<string, TreeSelection>;
  selectTreeNode: (taskId: string, selection: TreeSelection) => void;

  /** 技术日志展开偏好会被记住，但新用户永远从主要内容开始 (05 §4.3)。 */
  technicalLogOpen: boolean;
  setTechnicalLogOpen: (open: boolean) => void;

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
  densityMode: "comfortable",
  toggleDensity: () => set((s) => ({ densityMode: s.densityMode === "comfortable" ? "compact" : "comfortable" })),

  activeTab: {},
  setActiveTab: (taskId, tab) =>
    set((s) => ({ activeTab: { ...s.activeTab, [taskId]: tab } })),

  selectedRevision: {},
  setSelectedRevision: (taskId, rev) =>
    set((s) => ({ selectedRevision: { ...s.selectedRevision, [taskId]: rev } })),

  selectedRun: {},
  setSelectedRun: (taskId, runId) =>
    set((s) => ({ selectedRun: { ...s.selectedRun, [taskId]: runId } })),

  treeSelection: {},
  selectTreeNode: (taskId, selection) =>
    set((s) => ({
      treeSelection: { ...s.treeSelection, [taskId]: selection },
      selectedRevision: { ...s.selectedRevision, [taskId]: selection.revision },
      // Selecting a phase (no run) keeps whatever run the list had, so the content
      // pane never blanks out mid-navigation.
      selectedRun: selection.runId
        ? { ...s.selectedRun, [taskId]: selection.runId }
        : s.selectedRun,
    })),

  // 默认先展示可读结论与下一步；原始协议/命令日志按需展开（P1-04）。
  technicalLogOpen: false,
  setTechnicalLogOpen: (open) => set({ technicalLogOpen: open }),

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
