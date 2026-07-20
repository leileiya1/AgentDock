/** Small formatting helpers shared across the UI. */

export { cn } from "./utils";

const RELATIVE_UNITS: Array<[Intl.RelativeTimeFormatUnit, number]> = [
  ["year", 60 * 60 * 24 * 365],
  ["month", 60 * 60 * 24 * 30],
  ["day", 60 * 60 * 24],
  ["hour", 60 * 60],
  ["minute", 60],
];

const rtf = new Intl.RelativeTimeFormat("zh-CN", { numeric: "auto" });

/** ISO timestamp → "刚刚 / 2 分钟前 / 1 小时前". */
export function relativeTime(iso: string | null | undefined): string {
  if (!iso) return "";
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "";
  const diffSec = (then - Date.now()) / 1000;
  const abs = Math.abs(diffSec);
  if (abs < 45) return "刚刚";
  for (const [unit, secs] of RELATIVE_UNITS) {
    if (abs >= secs) {
      return rtf.format(Math.round(diffSec / secs), unit);
    }
  }
  return "刚刚";
}

/** Absolute local time, for tooltips. */
export function absoluteTime(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleString("zh-CN", { hour12: false });
}

export function shortSha(sha: string | null | undefined, len = 7): string {
  if (!sha) return "";
  return sha.slice(0, len);
}

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null) return "—";
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const value = bytes / Math.pow(1024, i);
  return `${value.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatSigned(n: number, sign: "+" | "-"): string {
  return `${sign}${n}`;
}

/** Copy text to clipboard; returns success. */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

export function taskCode(seq: number): string {
  return `TASK-${String(seq).padStart(3, "0")}`;
}
