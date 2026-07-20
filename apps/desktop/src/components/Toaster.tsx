import { AnimatePresence, motion } from "motion/react";
import { AlertCircle, Check } from "lucide-react";
import { useToastStore } from "@/stores/toastStore";
import { cn } from "@/lib/utils";

/** Bottom-right toast stack — errors + transient feedback only (02 §5). */
export function Toaster() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);
  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[100] flex max-w-sm flex-col gap-2">
      <AnimatePresence initial={false}>
        {toasts.map((t) => (
          <motion.button
            key={t.id}
            layout
            type="button"
            onClick={() => dismiss(t.id)}
            title="点击关闭"
            className={cn(
              "pointer-events-auto flex items-center gap-2 rounded-[var(--radius-control)] border px-3 py-2 text-left text-[13px] shadow-[var(--shadow-float)] glass",
              t.kind === "error" ? "border-bad/60 text-bad" : "border-line text-t1"
            )}
            initial={{ opacity: 0, x: 24, scale: 0.96 }}
            animate={{ opacity: 1, x: 0, scale: 1 }}
            exit={{ opacity: 0, x: 24, scale: 0.96 }}
            transition={{ type: "spring", stiffness: 420, damping: 32 }}
          >
            {t.kind === "error" ? (
              <AlertCircle className="size-4 shrink-0" />
            ) : (
              <Check className="size-4 shrink-0 text-ok" />
            )}
            <span>{t.message}</span>
          </motion.button>
        ))}
      </AnimatePresence>
    </div>
  );
}
