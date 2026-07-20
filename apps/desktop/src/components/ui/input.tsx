import * as React from "react";
import { cn } from "@/lib/utils";

export const Input = React.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(
  ({ className, ...props }, ref) => (
    <input
      ref={ref}
      data-slot="input"
      className={cn(
        "h-8 w-full rounded-[var(--radius-control)] border border-line bg-app/80 px-3 text-[13px] text-t1 shadow-inner shadow-black/20 transition-colors placeholder:text-t3 focus-visible:border-run/60 focus-visible:ring-2 focus-visible:ring-run/40 outline-none disabled:opacity-50",
        className
      )}
      {...props}
    />
  )
);
Input.displayName = "Input";

export const Textarea = React.forwardRef<
  HTMLTextAreaElement,
  React.TextareaHTMLAttributes<HTMLTextAreaElement>
>(({ className, ...props }, ref) => (
  <textarea
    ref={ref}
    data-slot="textarea"
    className={cn(
      "min-h-24 w-full resize-y rounded-[var(--radius-control)] border border-line bg-app/80 px-3 py-2 text-[13px] leading-relaxed text-t1 shadow-inner shadow-black/20 transition-colors placeholder:text-t3 focus-visible:border-run/60 focus-visible:ring-2 focus-visible:ring-run/40 outline-none disabled:opacity-50",
      className
    )}
    {...props}
  />
));
Textarea.displayName = "Textarea";
