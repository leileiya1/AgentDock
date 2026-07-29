import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "motion/react";
import { X } from "lucide-react";
import { trapTab } from "@/lib/focus";
import { cn } from "@/lib/utils";

interface Props {
  /** 抽屉模式下的开合状态；固定栏模式忽略。 */
  open: boolean;
  onClose: () => void;
  /** true = 抽屉（窄屏），false = 固定侧栏。 */
  drawer: boolean;
  title: string;
  width: string;
  children: ReactNode;
}

/**
 * 同一份内容的两种呈现 (05 §8): 宽屏是固定侧栏，窄屏收进抽屉。
 * 抽屉带焦点陷阱和 Escape 关闭，关闭后把焦点还给触发它的按钮。
 */
export function SidePanel({ open, onClose, drawer, title, width, children }: Props) {
  const panelRef = useRef<HTMLDivElement>(null);
  const restoreTo = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!drawer || !open) return;
    restoreTo.current = document.activeElement as HTMLElement | null;
    panelRef.current?.focus();

    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
        return;
      }
      // 焦点不能跑出抽屉，否则用户会在看不见的页面上按 Enter。
      trapTab(event, panelRef.current);
    };

    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      restoreTo.current?.focus?.();
    };
  }, [drawer, open, onClose]);

  if (!drawer) {
    return (
      <aside
        className="shrink-0 overflow-y-auto border-r border-line/70"
        style={{ width }}
        aria-label={title}
      >
        {children}
      </aside>
    );
  }

  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div
          className="fixed inset-0 z-40 bg-black/45"
          onMouseDown={onClose}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.15 }}
        >
          <motion.div
            ref={panelRef}
            role="dialog"
            aria-modal="true"
            aria-label={title}
            tabIndex={-1}
            onMouseDown={(e) => e.stopPropagation()}
            initial={{ x: "-100%" }}
            animate={{ x: 0 }}
            exit={{ x: "-100%" }}
            transition={{ type: "spring", stiffness: 420, damping: 38 }}
            className={cn(
              "flex h-full max-w-[85vw] flex-col overflow-hidden border-r border-line bg-panel shadow-[var(--shadow-float)] outline-none"
            )}
            style={{ width }}
          >
            <div className="flex shrink-0 items-center justify-between border-b border-line/70 px-3 py-2.5">
              <h2 className="text-[13px] font-semibold">{title}</h2>
              <button
                onClick={onClose}
                aria-label="关闭"
                className="grid size-6 place-items-center rounded-md text-t3 transition-colors hover:bg-raised hover:text-t1"
              >
                <X className="size-4" />
              </button>
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body
  );
}
