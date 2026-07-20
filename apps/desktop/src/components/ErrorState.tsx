import { AlertTriangle } from "lucide-react";
import { toAppError } from "@/copy/errors";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface Props {
  error: unknown;
  onRetry?: () => void;
  compact?: boolean;
}

/** Error surface for a failed query — never a white screen (02 §7). */
export function ErrorState({ error, onRetry, compact }: Props) {
  const d = toAppError(error);
  return (
    <div
      className={cn(
        "flex flex-col items-center gap-3 text-center text-t2",
        compact ? "px-4 py-6" : "px-6 py-12"
      )}
    >
      <div className="grid size-11 place-items-center rounded-full border border-bad/50 bg-bad/10 text-bad">
        <AlertTriangle className="size-5" />
      </div>
      <div className="text-[15px] font-medium text-t1">{d.title}</div>
      {(d.hint || d.detail) && <p className="max-w-md text-[13px] text-t2">{d.detail ?? d.hint}</p>}
      {onRetry && (
        <Button variant="outline" size="sm" onClick={onRetry} className="mt-1">
          重试
        </Button>
      )}
    </div>
  );
}
