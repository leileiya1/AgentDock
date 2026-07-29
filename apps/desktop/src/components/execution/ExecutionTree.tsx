import { useEffect, useMemo, useRef, useState } from "react";
import type { RevisionInfo } from "@/generated/bindings";
import type { Phase } from "@/copy/events";
import type { ExecutionTree as ExecutionTreeModel, RevisionNode } from "@/lib/execution/tree";
import { flattenTree, navigate, nodeId } from "@/lib/execution/treeNav";
import { cn } from "@/lib/utils";
import { CollapsedRevisionRow, PhaseRow, RevisionHeader, type TreeSelection } from "./TreeNodes";
import { SystemDetails } from "./SystemDetails";

interface Props {
  tree: ExecutionTreeModel;
  selection: TreeSelection | null;
  onSelect: (selection: TreeSelection) => void;
  /** 折叠行的改动摘要来自 TaskDetail.revisions。 */
  revisionStats?: RevisionInfo[];
}

const keyOf = (revision: number, phase: Phase) => `${revision}:${phase}`;

function durationSecs(node: RevisionNode): number | null {
  if (!node.startedAt || !node.endedAt) return null;
  return Math.max(0, Math.round((Date.parse(node.endedAt) - Date.parse(node.startedAt)) / 1000));
}

function statLine(info: RevisionInfo | undefined): string | null {
  if (!info?.stat) return null;
  const { files, insertions, deletions } = info.stat;
  return `${files} 文件 +${insertions} −${deletions}`;
}

/**
 * 执行树 (05 §3.1): revision → 业务阶段 → run 三层。默认展开当前 revision 和当前阶段，
 * 过去的 revision 折叠成一行总结；内部事件全部收进系统详情。
 */
export function ExecutionTree({ tree, selection, onSelect, revisionStats }: Props) {
  const [openRevisions, setOpenRevisions] = useState<Set<number>>(() => new Set([tree.currentRevision]));
  /**
   * Only explicit user toggles are stored. Anything untouched keeps following the
   * "expand the current phase" default, so a click early in the run does not freeze
   * the tree collapsed once the task moves on to the next phase.
   */
  const [phaseOverrides, setPhaseOverrides] = useState<Record<string, boolean>>({});
  // 每秒推进一次「已用时」，不依赖轮询后端 (05 §10 不用轮询制造闪烁)。
  const [now, setNow] = useState(() => Date.now());

  const hasRunning = useMemo(
    () => tree.revisions.some((r) => r.phases.some((p) => p.state === "running")),
    [tree]
  );

  useEffect(() => {
    if (!hasRunning) return;
    const timer = setInterval(() => setNow(Date.now()), 1_000);
    return () => clearInterval(timer);
  }, [hasRunning]);

  // 默认展开当前阶段：进行中的阶段，否则最后一个有内容的阶段。
  const defaultOpenPhases = useMemo(() => {
    const keys = new Set<string>();
    for (const revision of tree.revisions) {
      if (revision.revision !== tree.currentRevision) continue;
      const running = revision.phases.filter((p) => p.state === "running");
      const target = running.length > 0 ? running : revision.phases.filter((p) => p.state !== "pending").slice(-1);
      for (const phase of target) keys.add(keyOf(revision.revision, phase.phase));
    }
    return keys;
  }, [tree]);

  const isPhaseOpen = (revision: number, phase: Phase) => {
    const key = keyOf(revision, phase);
    return phaseOverrides[key] ?? defaultOpenPhases.has(key);
  };

  const togglePhase = (revision: number, phase: Phase) => {
    const key = keyOf(revision, phase);
    setPhaseOverrides((prev) => ({ ...prev, [key]: !isPhaseOpen(revision, phase) }));
  };

  const toggleRevision = (revision: number) => {
    setOpenRevisions((prev) => {
      const next = new Set(prev);
      if (next.has(revision)) next.delete(revision);
      else next.add(revision);
      return next;
    });
  };

  const statsByRevision = useMemo(
    () => new Map((revisionStats ?? []).map((info) => [info.revision, info])),
    [revisionStats]
  );

  // ── 键盘导航 (05 §8) ────────────────────────────────────────────────
  const treeRef = useRef<HTMLDivElement>(null);
  const [activeId, setActiveId] = useState<string | null>(null);

  const flat = useMemo(
    () =>
      flattenTree(tree, {
        isRevisionOpen: (revision) => openRevisions.has(revision),
        isPhaseOpen,
      }),
    // isPhaseOpen 依赖 phaseOverrides 与 defaultOpenPhases，两者都在依赖里。
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [tree, openRevisions, phaseOverrides, defaultOpenPhases]
  );

  // 焦点始终落在唯一的 roving tabindex 节点上；节点消失时回到第一个。
  const currentId = flat.some((node) => node.id === activeId) ? activeId : flat[0]?.id ?? null;

  const focusNode = (id: string) => {
    setActiveId(id);
    treeRef.current?.querySelector<HTMLElement>(`[data-tree-id="${CSS.escape(id)}"]`)?.focus();
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    const action = navigate(event.key, flat, currentId);
    if (!action) return;
    event.preventDefault();

    switch (action.type) {
      case "focus":
        focusNode(action.id);
        break;
      case "expand":
      case "collapse":
        if (action.node.kind === "revision") toggleRevision(action.node.revision);
        else if (action.node.phase) togglePhase(action.node.revision, action.node.phase);
        break;
      case "select":
        if (action.node.kind === "revision") toggleRevision(action.node.revision);
        else if (action.node.phase) {
          onSelect({
            revision: action.node.revision,
            phase: action.node.phase,
            runId: action.node.runId,
          });
        }
        break;
    }
  };

  return (
    <div className="flex flex-col px-1 py-2">
      {/* role="tree" 只包住真正的树节点——系统详情是独立的折叠区，
          放进树里会让读屏器把它念成一个 treeitem。 */}
      <div ref={treeRef} role="tree" aria-label="任务执行树" onKeyDown={onKeyDown}>
        <ol className="list-none" role="none">
        {tree.revisions.map((revision) => {
          const isCurrent = revision.revision === tree.currentRevision;
          const expanded = openRevisions.has(revision.revision);

          const revId = nodeId.revision(revision.revision);

          if (!expanded) {
            return (
              <CollapsedRevisionRow
                key={revision.revision}
                revision={revision.revision}
                conclusion={revision.conclusion}
                state={revision.state}
                durationSecs={durationSecs(revision)}
                stat={statLine(statsByRevision.get(revision.revision))}
                onExpand={() => toggleRevision(revision.revision)}
                treeId={revId}
                tabbable={currentId === revId}
                onFocusNode={() => setActiveId(revId)}
              />
            );
          }

          return (
            <li key={revision.revision} className={cn(!isCurrent && "opacity-90")} role="none">
              <RevisionHeader
                revision={revision.revision}
                isCurrent={isCurrent}
                conclusion={revision.conclusion}
                state={revision.state}
                startedAt={revision.startedAt}
                expanded
                onToggle={() => toggleRevision(revision.revision)}
                treeId={revId}
                tabbable={currentId === revId}
                onFocusNode={() => setActiveId(revId)}
              />
              <ol className="ml-3 list-none border-l border-line/70 pl-1" role="group">
                {revision.phases.map((phase) => (
                  <PhaseRow
                    key={phase.phase}
                    phase={phase}
                    revision={revision.revision}
                    expanded={isPhaseOpen(revision.revision, phase.phase)}
                    onToggle={() => togglePhase(revision.revision, phase.phase)}
                    selection={selection}
                    onSelectPhase={() => onSelect({ revision: revision.revision, phase: phase.phase, runId: null })}
                    onSelectRun={(runId) => onSelect({ revision: revision.revision, phase: phase.phase, runId })}
                    now={now}
                    currentId={currentId}
                    onFocusNode={setActiveId}
                  />
                ))}
              </ol>
            </li>
          );
        })}
        </ol>
      </div>

      <SystemDetails
        events={tree.systemEvents}
        onOpenRun={(event) => {
          const role = event.payload?.role;
          const phase = role === "planner" ? "plan" : role === "reviewer" ? "review" : role === "validator" ? "validate" : "develop";
          onSelect({ revision: event.revision ?? tree.currentRevision, phase, runId: event.runId });
        }}
      />
    </div>
  );
}
