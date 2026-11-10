import type { AgentKind, MemberRole, ProjectMemberInput } from "@/generated/bindings";
import { AgentMark } from "@/components/AgentMark";
import { agentLabel } from "@/copy/agents";
import { MAX_MEMBERS, ROLE_OPTIONS } from "@/lib/studio/studioOptions";
import { cn } from "@/lib/utils";

interface Props {
  value: ProjectMemberInput[];
  onChange: (next: ProjectMemberInput[]) => void;
  /** 能分配的 Agent（Provider 目录里的，含外部 sidecar）。 */
  candidates: AgentKind[];
  /** 最近任务里出现过的 Agent，一键填入用。 */
  recent: AgentKind[];
  nameOf: (agent: AgentKind) => string;
}

const iconBtn = "grid size-7 place-items-center rounded-row text-t2 hover:bg-app hover:text-t1 disabled:opacity-30";

/** 项目成员：小镇里这栋房子住谁、干什么、按什么顺序站。 */
export function MemberEditor({ value, onChange, candidates, recent, nameOf }: Props) {
  const taken = new Set(value.map((m) => m.agent));
  const full = value.length >= MAX_MEMBERS;
  const move = (i: number, by: number) => {
    const next = [...value];
    [next[i], next[i + by]] = [next[i + by], next[i]];
    onChange(next);
  };
  const setRole = (i: number, role: MemberRole) => onChange(value.map((m, j) => (j === i ? { ...m, role } : m)));
  const fillRecent = () => onChange(recent.slice(0, MAX_MEMBERS).map((agent, i) => ({ agent, role: i === 0 ? "developer" : "reviewer" })));

  return (
    <div className="flex flex-col gap-3">
      {value.length === 0 ? (
        <div className="flex flex-wrap items-center justify-between gap-2 rounded-control border border-dashed border-line-strong px-3 py-2.5 text-meta text-t2">
          <span>还没分配，小镇里按最近任务里出现的 Agent 来画。</span>
          {recent.length > 0 && (
            <button type="button" onClick={fillRecent} className="font-bold text-action hover:underline">
              按最近任务填入 {recent.length} 位
            </button>
          )}
        </div>
      ) : (
        <ol className="flex flex-col gap-1.5" aria-label="已分配的 Agent">
          {value.map((m, i) => (
            <li key={m.agent} className="flex items-center gap-2 rounded-control bg-app px-2 py-1.5 ring-1 ring-line">
              <AgentMark kind={m.agent} size={24} />
              <span className="min-w-0 flex-1 truncate text-meta font-bold text-t1" title={agentLabel(m.agent)}>
                {nameOf(m.agent)}
              </span>
              <span className="flex rounded-pill bg-raised p-0.5 ring-1 ring-line" role="radiogroup" aria-label={`${nameOf(m.agent)} 的分工`}>
                {ROLE_OPTIONS.map((r) => (
                  <button
                    key={r.value}
                    type="button"
                    role="radio"
                    aria-checked={m.role === r.value}
                    onClick={() => setRole(i, r.value)}
                    className={cn("rounded-pill px-2 py-0.5 text-micro font-bold", m.role === r.value ? "bg-action text-white" : "text-t2 hover:text-t1")}
                  >
                    {r.label}
                  </button>
                ))}
              </span>
              <button type="button" className={iconBtn} disabled={i === 0} onClick={() => move(i, -1)} aria-label={`${nameOf(m.agent)} 往前`}>
                ↑
              </button>
              <button type="button" className={iconBtn} disabled={i === value.length - 1} onClick={() => move(i, 1)} aria-label={`${nameOf(m.agent)} 往后`}>
                ↓
              </button>
              <button type="button" className={cn(iconBtn, "hover:text-status-danger")} onClick={() => onChange(value.filter((_, j) => j !== i))} aria-label={`移出 ${nameOf(m.agent)}`}>
                ×
              </button>
            </li>
          ))}
        </ol>
      )}

      <div className="flex flex-col gap-1.5">
        <span className="text-micro font-bold text-t3">
          {full ? `最多 ${MAX_MEMBERS} 位，先移出一位再加` : "点一下加入"}
        </span>
        <div className="flex flex-wrap gap-1.5">
          {candidates
            .filter((a) => !taken.has(a))
            .map((agent) => (
              <button
                key={agent}
                type="button"
                disabled={full}
                onClick={() => onChange([...value, { agent, role: "developer" }])}
                className="flex items-center gap-1.5 rounded-pill bg-raised py-0.5 pl-0.5 pr-2.5 text-micro font-bold text-t2 ring-1 ring-line hover:text-t1 disabled:opacity-40"
              >
                <AgentMark kind={agent} size={18} />+ {nameOf(agent)}
              </button>
            ))}
        </div>
      </div>
    </div>
  );
}
