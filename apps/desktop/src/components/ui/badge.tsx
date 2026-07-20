import * as React from "react";
import { cn } from "@/lib/utils";

/** Neutral pill for counts / small labels. Semantic state uses StateBadge. */
export function Badge({ className, ...props }: React.HTMLAttributes<HTMLSpanElement>) {
  return (
    <span
      data-slot="badge"
      className={cn(
        "inline-flex items-center gap-1 rounded-full bg-raised px-1.5 text-[10px] font-medium text-t3 tabular-nums",
        className
      )}
      {...props}
    />
  );
}
