import { useOnboarding } from "@/hooks/useEnv";
import { summarizeEnvironment } from "@/lib/envDots";
import { cn } from "@/lib/utils";

const TONE = {
  ok: "bg-ok",
  bad: "bg-bad",
  idle: "bg-idle",
} as const;

export function ProjectEnvironmentStatus({ onOpen }: { onOpen: () => void }) {
  const onboarding = useOnboarding();
  const summary = summarizeEnvironment(onboarding.data);

  return (
    <button
      type="button"
      onClick={onOpen}
      title={summary.detail}
      className="inline-flex items-center gap-1.5 text-meta text-t2 transition-colors hover:text-t1"
    >
      <span className={cn("size-1.5 rounded-circle", TONE[summary.tone])} aria-hidden />
      {summary.label}
    </button>
  );
}
