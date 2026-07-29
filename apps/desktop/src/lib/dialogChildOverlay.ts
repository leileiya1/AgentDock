const RECENT_CHILD_CLOSE_MS = 1_000;
let lastChildClosedAt = Number.NEGATIVE_INFINITY;

export function noteDialogChildClosed(now = Date.now()): void {
  lastChildClosedAt = now;
}

export function dialogChildClosedRecently(now = Date.now()): boolean {
  return now - lastChildClosedAt <= RECENT_CHILD_CLOSE_MS;
}
