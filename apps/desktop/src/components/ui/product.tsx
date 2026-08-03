import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/utils";

export function PageHeader({ title, meta, actions, className }: { title: ReactNode; meta?: ReactNode; actions?: ReactNode; className?: string }) {
  return (
    <header className={cn("flex min-h-16 items-center gap-4 border-b border-line px-6 py-3", className)}>
      <div className="min-w-0 flex-1">
        <h1 className="truncate text-page font-semibold text-t1">{title}</h1>
        {meta && <div className="mt-1 text-meta text-t3">{meta}</div>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </header>
  );
}

export function SectionHeader({ title, count, actions }: { title: ReactNode; count?: number; actions?: ReactNode }) {
  return (
    <div className="flex min-h-10 items-center gap-2 border-b border-line/70 px-3 py-2">
      <h2 className="min-w-0 flex-1 truncate text-section font-semibold text-t1">{title}</h2>
      {count != null && <span className="tabular-nums text-meta text-t3">{count}</span>}
      {actions}
    </div>
  );
}

export function Toolbar({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div role="toolbar" className={cn("flex min-h-10 items-center gap-2 border-y border-line/70 px-2 py-1.5", className)} {...props} />;
}

export function DataRow({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("grid min-h-10 grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-line/60 px-3 py-2", className)} {...props} />;
}

export function ActionRow({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("flex min-h-10 items-center justify-between gap-3 border-b border-line/60 px-3 py-2", className)} {...props} />;
}

export function InlineNotice({ title, children, tone = "neutral" }: { title: ReactNode; children?: ReactNode; tone?: "neutral" | "caution" | "danger" }) {
  const toneClass = tone === "danger"
    ? "border-status-danger/40 bg-status-human-bg/35"
    : tone === "caution" ? "border-caution/45 bg-caution-bg" : "border-line bg-panel";
  return (
    <div role={tone === "neutral" ? "status" : "alert"} className={cn("rounded-control border px-3 py-2 text-body", toneClass)}>
      <div className="font-medium text-t1">{title}</div>
      {children && <div className="mt-1 text-meta text-t2">{children}</div>}
    </div>
  );
}

export function Inspector({ title, children, className }: { title: ReactNode; children: ReactNode; className?: string }) {
  return (
    <aside aria-label={typeof title === "string" ? title : "检查器"} className={cn("min-w-0 border-l border-line bg-panel", className)}>
      <SectionHeader title={title} />
      <div className="min-h-0 overflow-y-auto p-3">{children}</div>
    </aside>
  );
}

export function SegmentedControl({ label, children }: { label: string; children: ReactNode }) {
  return <div role="tablist" aria-label={label} className="flex min-h-9 items-center rounded-control border border-line bg-panel p-1">{children}</div>;
}
