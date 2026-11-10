import { useMemo, useState } from "react";
import type { AgentKind, AgentProfile } from "@/generated/bindings";
import { Dialog } from "@/components/Dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { SkeletonRows } from "@/components/Skeleton";
import { PixelBot, botTint, memberShortLabel } from "@/components/pixel/PixelBot";
import { useAgentProfiles, useResetAgentProfile, useUpdateAgentProfile } from "@/hooks/useStudio";
import { useProviders } from "@/hooks/useProviders";
import { ALL_AGENTS, agentLabel } from "@/copy/agents";
import { ACCESSORY_OPTIONS, MAX_CATCHPHRASE, MAX_NICKNAME, blankProfile, displayName, isEmptyPatch, profilePatch, profileProblem } from "@/lib/studio/studioOptions";
import { ColorPicker, Field, Pill } from "@/components/studio/settings/Choice";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { cn } from "@/lib/utils";

/** Agent 档案：给每位 Agent 起名字、换颜色、戴配件、写口头禅。全局共用，换设备不丢。 */
export function AgentProfilesDialog({ open, onClose, initialAgent }: { open: boolean; onClose: () => void; initialAgent?: AgentKind }) {
  const profiles = useAgentProfiles();
  const providers = useProviders();
  const [picked, setPicked] = useState<AgentKind | null>(initialAgent ?? null);

  const agents = useMemo<AgentKind[]>(() => {
    const list = providers.data?.map((p) => p.id) ?? ALL_AGENTS;
    const extra = (profiles.data ?? []).map((p) => p.agent).filter((a) => !list.includes(a));
    return [...list, ...extra];
  }, [providers.data, profiles.data]);
  if (!open) return null;
  const current = picked ?? agents[0];
  const byAgent = new Map((profiles.data ?? []).map((p) => [p.agent, p]));

  return (
    <Dialog open onClose={onClose} title="Agent 档案" subtitle="名字和口头禅会出现在小屋和小镇的气泡里" width={720}>
      {profiles.isLoading || !current ? (
        <SkeletonRows rows={5} />
      ) : (
        <div className="flex min-h-[360px] gap-4">
          <ul className="flex w-48 shrink-0 flex-col gap-0.5 overflow-y-auto border-r border-line pr-2" aria-label="选择 Agent">
            {agents.map((agent) => {
              const p = byAgent.get(agent);
              return (
                <li key={agent}>
                  <button
                    type="button"
                    aria-current={agent === current}
                    onClick={() => setPicked(agent)}
                    className={cn("flex w-full items-center gap-2 rounded-row px-2 py-1.5 text-left", agent === current ? "bg-action-bg text-t1" : "text-t2 hover:bg-app hover:text-t1")}
                  >
                    <PixelBot member={agent} size={22} decorative color={p?.color} />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-meta font-bold">{displayName(p, agentLabel(agent))}</span>
                      {p?.nickname && <span className="block truncate text-micro text-t3">{agentLabel(agent)}</span>}
                    </span>
                    {p && <span className="size-1.5 rounded-circle bg-action" aria-label="改过" />}
                  </button>
                </li>
              );
            })}
          </ul>
          <ProfileForm key={`${current}:${byAgent.get(current)?.updatedAt ?? ""}`} agent={current} initial={byAgent.get(current) ?? blankProfile(current)} saved={byAgent.has(current)} />
        </div>
      )}
    </Dialog>
  );
}

function ProfileForm({ agent, initial, saved }: { agent: AgentKind; initial: AgentProfile; saved: boolean }) {
  const [draft, setDraft] = useState(initial);
  const update = useUpdateAgentProfile();
  const reset = useResetAgentProfile();
  const set = (part: Partial<AgentProfile>) => setDraft((d) => ({ ...d, ...part }));
  const patch = profilePatch(initial, draft);
  const problem = profileProblem(draft);
  const name = displayName(draft, memberShortLabel(agent));

  const save = async () => {
    try {
      await update.mutateAsync({ agent, patch });
      toast.info(`${name} 的档案存好了`);
    } catch (error) {
      toast.error(errorLine(error));
    }
  };
  const restore = async () => {
    try {
      await reset.mutateAsync(agent);
      setDraft(blankProfile(agent));
      toast.info(`${memberShortLabel(agent)} 恢复默认了`);
    } catch (error) {
      toast.error(errorLine(error));
    }
  };

  return (
    <div className="flex min-w-0 flex-1 flex-col gap-4">
      <div className="flex items-center gap-3 rounded-card p-3 ring-1 ring-line" style={{ background: botTint(agent) }}>
        <span className="relative">
          <PixelBot member={agent} mode="work" size={56} decorative color={draft.color} />
        </span>
        <div className="min-w-0">
          <div className="text-body font-extrabold text-t1">{name}</div>
          <div className="text-micro text-t2">
            {agentLabel(agent)}
            {draft.accessory && draft.accessory !== "none" && ` · 戴着${ACCESSORY_OPTIONS.find((a) => a.value === draft.accessory)?.label}`}
          </div>
          {draft.catchphrase?.trim() && <div className="mt-1 inline-block rounded-row bg-raised px-2 py-0.5 text-meta text-t1 shadow-press">「{draft.catchphrase.trim()}」</div>}
        </div>
      </div>

      <Field label="名字" hint={`最多 ${MAX_NICKNAME} 个字，留空用默认名`}>
        <Input value={draft.nickname ?? ""} onChange={(e) => set({ nickname: e.target.value })} placeholder={memberShortLabel(agent)} aria-label="名字" />
      </Field>
      <Field label="机身颜色">
        <ColorPicker label="机身颜色" value={draft.color} onChange={(color) => set({ color })} />
      </Field>
      <Field label="配件">
        <div className="flex flex-wrap gap-1.5" role="radiogroup" aria-label="配件">
          {ACCESSORY_OPTIONS.map((a) => (
            <Pill key={a.value} on={(draft.accessory ?? "none") === a.value} onClick={() => set({ accessory: a.value === "none" ? null : a.value })}>
              {a.label}
            </Pill>
          ))}
        </div>
      </Field>
      <Field label="口头禅" hint={`最多 ${MAX_CATCHPHRASE} 个字`}>
        <Input value={draft.catchphrase ?? ""} onChange={(e) => set({ catchphrase: e.target.value })} placeholder="比如：这个交给我" aria-label="口头禅" />
      </Field>

      <div className="mt-auto flex items-center justify-end gap-2 border-t border-line pt-3">
        {problem && <span className="mr-auto text-meta text-status-danger">{problem}</span>}
        <Button variant="ghost" onClick={restore} disabled={!saved || reset.isPending}>恢复默认</Button>
        <Button variant="primary" onClick={save} disabled={isEmptyPatch(patch) || !!problem || update.isPending}>
          {update.isPending ? "保存中…" : "保存"}
        </Button>
      </div>
    </div>
  );
}
