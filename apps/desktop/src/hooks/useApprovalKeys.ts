import { useEffect, useRef } from "react";
import { shouldIgnoreKey } from "@/lib/keys";

/** 等你批准时：A 打开批准确认框，R 打开驳回框。只开框，不直接批准。 */
export function useApprovalKeys({ onApprove, onReject, enabled }: { onApprove: () => void; onReject: () => void; enabled: boolean }): void {
  const latest = useRef({ onApprove, onReject });
  latest.current = { onApprove, onReject };
  useEffect(() => {
    if (!enabled) return;
    const onKey = (e: KeyboardEvent) => {
      if (shouldIgnoreKey(e) || e.shiftKey) return;
      const k = e.key.toLowerCase();
      if (k === "a") {
        e.preventDefault();
        latest.current.onApprove();
      } else if (k === "r") {
        e.preventDefault();
        latest.current.onReject();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [enabled]);
}
