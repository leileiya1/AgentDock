import { useEffect, useState } from "react";

export function useDelayedPending(pending: boolean, delayMs = 400): boolean {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    if (!pending) {
      setVisible(false);
      return;
    }
    const timer = window.setTimeout(() => setVisible(true), delayMs);
    return () => window.clearTimeout(timer);
  }, [delayMs, pending]);

  return visible;
}
