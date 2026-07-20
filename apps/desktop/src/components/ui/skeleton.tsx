import { cn } from "@/lib/utils";

export function Skeleton({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      data-slot="skeleton"
      className={cn(
        "rounded-md bg-[linear-gradient(90deg,var(--color-panel),var(--color-raised),var(--color-panel))] bg-[length:200%_100%] animate-shimmer",
        className
      )}
      {...props}
    />
  );
}
