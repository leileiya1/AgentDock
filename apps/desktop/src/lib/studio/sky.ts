/** 实时昼夜：按本机时间给出用哪套画面、要不要叠一层晨昏色，以及一个时段名。 */

export type SkyName = "predawn" | "dawn" | "day" | "dusk" | "night";

export interface SkyPhase {
  name: SkyName;
  /** 用白天还是夜间那套像素画。 */
  theme: "day" | "night";
  label: string;
  /** 叠在画面上的颜色 token 和强度（0 = 不叠）。 */
  tint: { token: string; alpha: number } | null;
  /** 夜里没活干的机器人回屋睡觉。 */
  sleepy: boolean;
}

const ramp = (v: number, from: number, to: number) => Math.min(1, Math.max(0, (v - from) / (to - from)));

/**
 * 06–07 黎明前（夜景 + 淡紫）、07–08 清晨（白天 + 渐淡的桃色）、08–17 白天、
 * 17–19 傍晚（白天 + 越来越浓的橘色）、19–06 夜晚。
 */
export function skyPhase(now: Date): SkyPhase {
  const h = now.getHours() + now.getMinutes() / 60;
  if (h >= 6 && h < 7) return { name: "predawn", theme: "night", label: "黎明前", tint: { token: "--color-sky-predawn", alpha: 0.18 * ramp(h, 6, 7) }, sleepy: true };
  if (h >= 7 && h < 8) return { name: "dawn", theme: "day", label: "清晨", tint: { token: "--color-sky-dawn", alpha: 0.22 * (1 - ramp(h, 7, 8)) }, sleepy: false };
  if (h >= 8 && h < 17) return { name: "day", theme: "day", label: "白天", tint: null, sleepy: false };
  if (h >= 17 && h < 19) return { name: "dusk", theme: "day", label: "傍晚", tint: { token: "--color-sky-dusk", alpha: 0.08 + 0.22 * ramp(h, 17, 19) }, sleepy: false };
  return { name: "night", theme: "night", label: h >= 23 || h < 6 ? "深夜" : "夜晚", tint: null, sleepy: true };
}

/** 手动选了白天 / 夜间就不叠色、不赶人回家。 */
export function skyFor(choice: "auto" | "day" | "night", now: Date): SkyPhase {
  if (choice === "auto") return skyPhase(now);
  return choice === "day"
    ? { name: "day", theme: "day", label: "白天", tint: null, sleepy: false }
    : { name: "night", theme: "night", label: "夜间", tint: null, sleepy: false };
}
