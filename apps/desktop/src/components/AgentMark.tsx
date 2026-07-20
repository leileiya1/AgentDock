import type { AgentKind, Actor } from "@/generated/bindings";
import { agentLabel } from "@/copy/agents";
import { ProviderIcon } from "@/components/ProviderIcon";
import { cn } from "@/lib/utils";

interface Props {
  kind: AgentKind;
  size?: number;
  title?: string;
}

const base =
  "inline-grid place-items-center shrink-0 rounded-full border font-mono font-semibold leading-none";

/** Reuse the Provider artwork in tasks and timelines for immediate recognition. */
export function AgentMark({ kind, size = 24, title }: Props) {
  return (
    <span
      title={title ?? agentLabel(kind)}
      aria-label={agentLabel(kind)}
    >
      <ProviderIcon provider={kind} size={size} className="rounded-full" />
    </span>
  );
}

const ACTOR_MARK: Record<Actor, { mark: string; color: string; label: string }> = {
  orchestrator: { mark: "⚙", color: "var(--color-t2)", label: "调度" },
  agent: { mark: "•", color: "var(--color-t2)", label: "Agent" },
  human: { mark: "你", color: "var(--color-human)", label: "你" },
  system: { mark: "◇", color: "var(--color-t3)", label: "系统" },
};

/** Mark for a timeline actor (orchestrator gear / agent / human / system). */
export function ActorMark({
  actor,
  agent,
  size = 22,
}: {
  actor: Actor;
  agent?: AgentKind | null;
  size?: number;
}) {
  if (actor === "agent" && agent) return <AgentMark kind={agent} size={size} />;
  const meta = ACTOR_MARK[actor];
  return (
    <span
      className={cn(base, "font-sans")}
      title={meta.label}
      aria-label={meta.label}
      style={{
        width: size,
        height: size,
        fontSize: actor === "human" ? size * 0.5 : size * 0.62,
        color: meta.color,
        borderColor: "var(--color-line)",
        background: actor === "human" ? "var(--color-human-bg)" : "var(--color-raised)",
      }}
    >
      {meta.mark}
    </span>
  );
}
