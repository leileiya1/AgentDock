/**
 * 8-bit 提示音：用 WebAudio 现场合成方波，不带任何音频文件。
 * 开关只存在本机（localStorage），默认关，免得吓人。
 */
const KEY = "agentflow.chime";
const listeners = new Set<(on: boolean) => void>();

export type ChimeKind = "done" | "attention" | "failed";

/** 每种事件一段小旋律：[频率 Hz, 时长 s]。 */
export const TUNES: Record<ChimeKind, Array<[number, number]>> = {
  done: [[659, 0.08], [784, 0.08], [1047, 0.16]], // E5 G5 C6：叮叮咚，上扬
  attention: [[880, 0.1], [0, 0.05], [880, 0.1]], // A5 两下：敲门
  failed: [[392, 0.12], [311, 0.2]], // G4 → D#4：下沉
};

export function chimeEnabled(): boolean {
  try {
    return localStorage.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

export function setChimeEnabled(on: boolean): void {
  try {
    localStorage.setItem(KEY, on ? "1" : "0");
  } catch {
    /* per-viewer convenience only */
  }
  listeners.forEach((fn) => fn(on));
  if (on) playChime("done");
}

export function onChimeChange(fn: (on: boolean) => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

let ctx: AudioContext | null = null;
let lastAt = 0;

export function playChime(kind: ChimeKind): void {
  if (typeof window === "undefined" || !("AudioContext" in window)) return;
  // 一批任务同时变化时只响一次。
  const now = Date.now();
  if (now - lastAt < 600) return;
  lastAt = now;
  try {
    ctx ??= new AudioContext();
    let t = ctx.currentTime + 0.01;
    for (const [freq, dur] of TUNES[kind]) {
      if (freq > 0) {
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = "square";
        osc.frequency.value = freq;
        gain.gain.setValueAtTime(0.06, t);
        gain.gain.exponentialRampToValueAtTime(0.001, t + dur);
        osc.connect(gain).connect(ctx.destination);
        osc.start(t);
        osc.stop(t + dur);
      }
      t += dur;
    }
  } catch {
    /* audio is decoration */
  }
}
