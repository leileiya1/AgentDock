const FOCUSABLE = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

/**
 * Focusable descendants in DOM order, used by the modal 焦点陷阱 (05 §8).
 * `offsetParent === null` filters out collapsed sections; the active element is kept
 * regardless so a focused-but-hidden node cannot strand the trap.
 */
export function focusableWithin(root: HTMLElement | null): HTMLElement[] {
  if (!root) return [];
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => el.offsetParent !== null || el === document.activeElement
  );
}

/**
 * Shared Tab handling for modal surfaces: wrap focus at both ends.
 * Returns true when the event was handled.
 */
export function trapTab(event: KeyboardEvent, container: HTMLElement | null): boolean {
  if (event.key !== "Tab") return false;
  const focusables = focusableWithin(container);
  if (focusables.length === 0) return false;

  const first = focusables[0];
  const last = focusables[focusables.length - 1];
  const active = document.activeElement;

  if (event.shiftKey && (active === first || active === container)) {
    event.preventDefault();
    last.focus();
    return true;
  }
  if (!event.shiftKey && active === last) {
    event.preventDefault();
    first.focus();
    return true;
  }
  return false;
}
