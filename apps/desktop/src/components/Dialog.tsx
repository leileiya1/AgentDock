import { useEffect, useRef, type CSSProperties, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "motion/react";
import { trapTab } from "@/lib/focus";
import { PixelIcon } from "@/components/pixel/PixelIcon";
import { dialogChildClosedRecently } from "@/lib/dialogChildOverlay";

interface Props {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  footer?: ReactNode;
  width?: number;
  /** allow Cmd/Ctrl+Enter to trigger the confirm action */
  onConfirmKey?: () => void;
  /** Avatar or icon tile shown before the title (a squad member, an action icon). */
  leading?: ReactNode;
  /** Short line under the title, e.g. who is asking. */
  subtitle?: ReactNode;
  /** CSS background for the header band, e.g. the asking robot's station tint. */
  headerTint?: string;
}

const OPEN_DIALOG_CHILD_SELECTOR = '[data-slot="select-content"]';
const DIALOG_CHILD_TRIGGER_SELECTOR = '[data-slot="select-trigger"]';

type DialogEscapeRoot = {
  querySelector: (selector: string) => unknown | null;
  activeElement?: { closest?: (selector: string) => unknown | null } | null;
};

export function shouldDeferDialogEscape(root: DialogEscapeRoot): boolean {
  if (root.querySelector(OPEN_DIALOG_CHILD_SELECTOR) !== null) return true;
  // macOS can dismiss the native accessibility popup before this window-level
  // listener runs. Radix restores focus to the trigger, which is the remaining
  // signal that this Escape belongs to the child Select rather than the dialog.
  return root.activeElement?.closest?.(DIALOG_CHILD_TRIGGER_SELECTOR) != null || dialogChildClosedRecently();
}

/** Modal with short fade/translate motion; Escape and explicit controls dismiss. */
export function Dialog({ open, onClose, title, children, footer, width = 480, onConfirmKey, leading, subtitle, headerTint }: Props) {
  const panelRef = useRef<HTMLDivElement>(null);
  // Keep the latest callbacks without restarting the focus-trap effect. Dialog
  // callers commonly pass inline functions, so depending on those identities
  // would focus the panel again after every controlled-input keystroke.
  const onCloseRef = useRef(onClose);
  const onConfirmKeyRef = useRef(onConfirmKey);
  onCloseRef.current = onClose;
  onConfirmKeyRef.current = onConfirmKey;

  useEffect(() => {
    if (!open) return;
    // 关闭后焦点必须回到触发它的按钮，否则键盘用户会掉回文档开头 (05 §8)。
    const restoreTo = document.activeElement as HTMLElement | null;

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        // Radix Select renders its content in a portal. Let the child overlay
        // consume Escape first; otherwise the same key also closes this parent
        // dialog and discards the user's in-progress form.
        if (shouldDeferDialogEscape(document)) return;
        e.preventDefault();
        onCloseRef.current();
        return;
      }
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter" && onConfirmKeyRef.current) {
        e.preventDefault();
        onConfirmKeyRef.current();
        return;
      }
      // 焦点陷阱：Tab 不能跑到弹窗背后那层看不见的页面上。
      trapTab(e, panelRef.current);
    };

    // Capture before portaled Radix children synchronously unmount themselves.
    // In the bubble phase the Select is already gone, so Escape is otherwise
    // indistinguishable from a request to close the parent dialog.
    window.addEventListener("keydown", onKey, true);
    panelRef.current?.focus();
    return () => {
      window.removeEventListener("keydown", onKey, true);
      restoreTo?.focus?.();
    };
  }, [open]);

  // SSR tests do not have a portal host. Render the semantic shell inline so
  // feature dialogs can still be regression-tested without changing runtime UI.
  if (typeof document === "undefined") {
    return open ? <div role="dialog" aria-label={title}>{children}{footer}</div> : null;
  }

  // Render outside animated parents: their CSS transform would otherwise make
  // position: fixed relative to the parent action bar instead of the viewport.
  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div
          className="fixed inset-0 z-50 grid place-items-center bg-t1/35 p-5 backdrop-blur-[2px]"
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
            style={{ "--dialog-width": `${width}px` } as CSSProperties}
            className="relative flex max-h-[90vh] w-[min(var(--dialog-width),calc(100vw-2.5rem))] flex-col overflow-hidden rounded-overlay border border-line bg-raised shadow-[var(--shadow-float)] outline-none max-sm:h-full max-sm:max-h-full max-sm:w-full max-sm:rounded-none"
            onMouseDown={(e) => e.stopPropagation()}
            initial={{ opacity: 0, y: 14, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 6 }}
            transition={{ duration: 0.24, ease: [0.2, 1.2, 0.4, 1] }}
          >
            <div
              className="flex items-center gap-3 border-b border-line/70 px-5 py-3.5"
              style={headerTint ? { background: headerTint } : undefined}
            >
              {leading}
              <div className="min-w-0 flex-1">
                <h2 className="truncate text-section font-extrabold text-t1">{title}</h2>
                {subtitle && <div className="mt-0.5 truncate text-meta text-t2">{subtitle}</div>}
              </div>
              <button
                onClick={onClose}
                className="grid size-8 shrink-0 place-items-center rounded-control bg-raised/70 text-t2 transition-colors hover:bg-raised hover:text-t1"
                aria-label="关闭"
              >
                <PixelIcon name="cross" size={10} />
              </button>
            </div>
            <div className="overflow-y-auto px-5 py-4">{children}</div>
            {footer && (
              <div className="flex justify-end gap-2 border-t border-line/70 bg-app px-5 py-3">{footer}</div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}
