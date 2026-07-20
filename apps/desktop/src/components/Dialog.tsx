import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "motion/react";
import { X } from "lucide-react";

interface Props {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  footer?: ReactNode;
  width?: number;
  /** allow Cmd/Ctrl+Enter to trigger the confirm action */
  onConfirmKey?: () => void;
}

/** Glass modal with Motion enter/exit + spring; Escape / overlay dismiss. */
export function Dialog({ open, onClose, title, children, footer, width = 480, onConfirmKey }: Props) {
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if ((e.metaKey || e.ctrlKey) && e.key === "Enter" && onConfirmKey) {
        e.preventDefault();
        onConfirmKey();
      }
    };
    window.addEventListener("keydown", onKey);
    panelRef.current?.focus();
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose, onConfirmKey]);

  // Render outside animated parents: their CSS transform would otherwise make
  // position: fixed relative to the parent action bar instead of the viewport.
  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div
          className="fixed inset-0 z-50 grid place-items-center p-5 bg-black/55 backdrop-blur-[3px]"
          onMouseDown={onClose}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.16 }}
        >
          <motion.div
            ref={panelRef}
            role="dialog"
            aria-modal="true"
            aria-label={title}
            tabIndex={-1}
            style={{ width }}
            className="relative flex max-h-[90vh] max-w-full flex-col overflow-hidden rounded-[var(--radius-panel)] border border-line/80 glass shadow-[var(--shadow-float)] outline-none"
            onMouseDown={(e) => e.stopPropagation()}
            initial={{ opacity: 0, y: 12, scale: 0.97 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 8, scale: 0.98 }}
            transition={{ type: "spring", stiffness: 380, damping: 30 }}
          >
            {/* top light seam for depth */}
            <span className="pointer-events-none absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-run/25 to-transparent" />
            <div className="flex items-center justify-between border-b border-line/70 px-4 py-3">
              <h2 className="text-[15px] font-semibold text-t1">{title}</h2>
              <button
                onClick={onClose}
                className="grid size-6 place-items-center rounded-md text-t3 transition-colors hover:bg-raised hover:text-t1"
                aria-label="关闭"
              >
                <X className="size-4" />
              </button>
            </div>
            <div className="overflow-y-auto px-4 py-4">{children}</div>
            {footer && (
              <div className="flex justify-end gap-2 border-t border-line/70 px-4 py-3">{footer}</div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}
