import { useMemo, useState } from "react";
import type { AgentKind, ProjectAppearance, ProjectMember, ProjectMemberInput, ProjectSummary } from "@/generated/bindings";
import { Dialog } from "@/components/Dialog";
import { Button } from "@/components/ui/button";
import { SkeletonRows } from "@/components/Skeleton";
import { useAgentProfiles, useProjectAppearance, useProjectMembers, useSetProjectMembers, useUpdateProjectAppearance } from "@/hooks/useStudio";
import { useProviders } from "@/hooks/useProviders";
import { ALL_AGENTS } from "@/copy/agents";
import { memberShortLabel } from "@/components/pixel/PixelBot";
import { BUILDINGS, PROJECT_KINDS, guessKind } from "@/lib/studio/buildings";
import { MAX_YARD, SIGN_OPTIONS, YARD_OPTIONS, appearancePatch, displayName, isEmptyPatch, sameMembers } from "@/lib/studio/studioOptions";
import { ColorPicker, Field, Pill } from "@/components/studio/settings/Choice";
import { MemberEditor } from "@/components/studio/settings/MemberEditor";
import { errorLine } from "@/copy/errors";
import { toast } from "@/stores/toastStore";
import { cn } from "@/lib/utils";

interface Props {
  open: boolean;
  onClose: () => void;
  projectId: string;
  projectName: string;
  summary: ProjectSummary | undefined;
}

/** 装修房子：选造型、屋顶颜色、门牌、院子摆件，以及这个项目住哪几位 Agent。 */
export function ProjectDecorDialog(props: Props) {
  const appearance = useProjectAppearance();
  const members = useProjectMembers(props.open ? props.projectId : undefined);
  if (!props.open) return null;
  const look = appearance.data?.find((a) => a.projectId === props.projectId);
  if (!look || !members.data) {
    return (
      <Dialog open onClose={props.onClose} title={`装修 ${props.projectName}`} width={620}>
        <SkeletonRows rows={5} />
      </Dialog>
    );
  }
  return <DecorForm key={props.projectId} {...props} initialLook={look} initialMembers={members.data} />;
}

function DecorForm({ onClose, projectId, projectName, summary, initialLook, initialMembers }: Props & { initialLook: ProjectAppearance; initialMembers: ProjectMember[] }) {
  const [look, setLook] = useState(initialLook);
  const [crew, setCrew] = useState<ProjectMemberInput[]>(() => initialMembers.map(({ agent, role }) => ({ agent, role })));
  const providers = useProviders();
  const profiles = useAgentProfiles();
  const saveLook = useUpdateProjectAppearance();
  const saveCrew = useSetProjectMembers();
  const guessed = guessKind(projectName);

  const candidates = useMemo<AgentKind[]>(() => providers.data?.map((p) => p.id) ?? ALL_AGENTS, [providers.data]);
  const nameOf = (agent: AgentKind) => displayName(profiles.data?.find((p) => p.agent === agent), memberShortLabel(agent));
  const set = (part: Partial<ProjectAppearance>) => setLook((l) => ({ ...l, ...part }));
  const toggleYard = (item: string) =>
    set({ yard: look.yard.includes(item) ? look.yard.filter((y) => y !== item) : [...look.yard, item] });

  const patch = appearancePatch(initialLook, look);
  const crewChanged = !sameMembers(initialMembers, crew);
  const dirty = !isEmptyPatch(patch) || crewChanged;
  const busy = saveLook.isPending || saveCrew.isPending;

  const save = async () => {
    try {
      if (!isEmptyPatch(patch)) await saveLook.mutateAsync({ projectId, patch });
      if (crewChanged) await saveCrew.mutateAsync({ projectId, members: crew });
      toast.info(`${projectName} 装修好了`);
      onClose();
    } catch (error) {
      toast.error(errorLine(error));
    }
  };

  return (
    <Dialog
      open
      onClose={onClose}
      title={`装修 ${projectName}`}
      subtitle="改的是小镇里这栋房子的样子和住户，不影响任务本身"
      width={640}
      onConfirmKey={dirty ? save : undefined}
      footer={
        <>
          <Button variant="outline" onClick={onClose}>取消</Button>
          <Button variant="primary" onClick={save} disabled={!dirty || busy}>
            {busy ? "保存中…" : "保存"}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-5">
        <Field label="房子造型" hint={look.kind ? undefined : `没选，按名字猜的是「${BUILDINGS[guessed].building}」`}>
          <div className="grid grid-cols-2 gap-1.5 sm:grid-cols-4" role="radiogroup" aria-label="房子造型">
            {PROJECT_KINDS.map((kind) => {
              const b = BUILDINGS[kind];
              const on = look.kind === kind;
              return (
                <button
                  key={kind}
                  type="button"
                  role="radio"
                  aria-checked={on}
                  title={b.fits}
                  aria-label={`${b.building}（${b.label}）`}
                  onClick={() => set({ kind: on ? null : kind })}
                  className={cn(
                    "flex flex-col items-start rounded-control px-2.5 py-2 text-left ring-1 transition-colors",
                    on ? "bg-action-bg ring-action" : "bg-raised ring-line hover:ring-line-strong",
                    !look.kind && kind === guessed && "outline-1 outline-offset-1 outline-dashed outline-action/60"
                  )}
                >
                  <span className="text-meta font-extrabold text-t1">{b.building}</span>
                  <span className="text-micro text-t2">{b.label}</span>
                </button>
              );
            })}
          </div>
        </Field>

        <Field label="屋顶颜色">
          <ColorPicker label="屋顶颜色" value={look.roof} onChange={(roof) => set({ roof })} />
        </Field>

        <Field label="门牌">
          <div className="flex flex-wrap gap-1.5" role="radiogroup" aria-label="门牌">
            <Pill on={look.sign === null} onClick={() => set({ sign: null })}>默认</Pill>
            {SIGN_OPTIONS.map((s) => (
              <Pill key={s.value} on={look.sign === s.value} onClick={() => set({ sign: s.value })}>
                {s.label}
              </Pill>
            ))}
          </div>
        </Field>

        <Field label="院子摆件" hint={`${look.yard.length} / ${MAX_YARD}`}>
          <div className="flex flex-wrap gap-1.5">
            {YARD_OPTIONS.map((y) => {
              const on = look.yard.includes(y.value);
              return (
                <Pill key={y.value} role="checkbox" on={on} disabled={!on && look.yard.length >= MAX_YARD} onClick={() => toggleYard(y.value)}>
                  {y.label}
                </Pill>
              );
            })}
          </div>
        </Field>

        <Field label="住户（分配的 Agent）" hint="小镇里就画这几位，顺序就是站位">
          <MemberEditor value={crew} onChange={setCrew} candidates={candidates} recent={summary?.agents ?? []} nameOf={nameOf} />
        </Field>
      </div>
    </Dialog>
  );
}
