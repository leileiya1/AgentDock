import { useEffect, useState } from "react";
import { skyFor, type SkyPhase } from "@/lib/studio/sky";

/** 每分钟看一次表；开着小镇过了傍晚，画面会自己慢慢变成夜景。 */
export function useSky(choice: "auto" | "day" | "night"): SkyPhase {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    if (choice !== "auto") return;
    const id = window.setInterval(() => setNow(new Date()), 60_000);
    return () => window.clearInterval(id);
  }, [choice]);
  return skyFor(choice, now);
}
