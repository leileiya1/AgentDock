import type { AgentProfile, AgentProfilePatch, MemberRole, ProjectAppearance, ProjectAppearancePatch, ProjectMember, ProjectMemberInput } from "@/generated/bindings";

/**
 * 小镇装修与 Agent 档案的可选项和小工具。取值必须和后端 `studio_profiles.rs` 的白名单一致，
 * 文案给用户看。
 */

export const SIGN_OPTIONS = [
  { value: "wood", label: "木牌" },
  { value: "neon", label: "霓虹" },
  { value: "stone", label: "石碑" },
] as const;

export const YARD_OPTIONS = [
  { value: "fountain", label: "喷泉" },
  { value: "flag", label: "旗杆" },
  { value: "statue", label: "雕像" },
  { value: "garden", label: "花圃" },
  { value: "bench", label: "长椅" },
  { value: "pet", label: "宠物" },
] as const;

export const ACCESSORY_OPTIONS = [
  { value: "none", label: "不戴" },
  { value: "cap", label: "鸭舌帽" },
  { value: "headphones", label: "耳机" },
  { value: "glasses", label: "眼镜" },
  { value: "bowtie", label: "领结" },
  { value: "antenna", label: "天线" },
  { value: "scarf", label: "围巾" },
] as const;

export const ROLE_OPTIONS: ReadonlyArray<{ value: MemberRole; label: string }> = [
  { value: "developer", label: "写代码" },
  { value: "reviewer", label: "审代码" },
  { value: "both", label: "都干" },
];

export const MAX_YARD = 6;
export const MAX_MEMBERS = 12;
export const MAX_NICKNAME = 16;
export const MAX_CATCHPHRASE = 40;

/** 预设色板复用机器人家族色（主题 token），选中时换算成 `#rrggbb` 存到后端。 */
export const SWATCH_TOKENS = ["claude", "codex", "qoder", "gemini", "qwen", "grok", "deepseek", "kimi", "minimax"].map(
  (f) => `--color-bot-${f}`
);

export function resolveToken(token: string): string | null {
  try {
    const v = getComputedStyle(document.documentElement).getPropertyValue(token).trim().toLowerCase();
    return /^#[0-9a-f]{6}$/.test(v) ? v : null;
  } catch {
    return null;
  }
}

export const isHex = (v: string) => /^#[0-9a-fA-F]{6}$/.test(v);

/** 只把改过的字段放进 patch：没改的不传，清空的传 null。 */
export function appearancePatch(before: ProjectAppearance, after: ProjectAppearance): ProjectAppearancePatch {
  const patch: ProjectAppearancePatch = {};
  if (before.kind !== after.kind) patch.kind = after.kind;
  if (before.roof !== after.roof) patch.roof = after.roof;
  if (before.sign !== after.sign) patch.sign = after.sign;
  if (before.yard.join() !== after.yard.join()) patch.yard = after.yard;
  return patch;
}

export function profilePatch(before: AgentProfile, after: AgentProfile): AgentProfilePatch {
  const patch: AgentProfilePatch = {};
  const norm = (v: string | null) => (v?.trim() ? v.trim() : null);
  if (norm(before.nickname) !== norm(after.nickname)) patch.nickname = norm(after.nickname);
  if (before.color !== after.color) patch.color = after.color;
  if (before.accessory !== after.accessory) patch.accessory = after.accessory;
  if (norm(before.catchphrase) !== norm(after.catchphrase)) patch.catchphrase = norm(after.catchphrase);
  return patch;
}

export const isEmptyPatch = (p: object) => Object.keys(p).length === 0;

export function sameMembers(a: readonly ProjectMember[], b: readonly ProjectMemberInput[]): boolean {
  return a.length === b.length && a.every((m, i) => m.agent === b[i].agent && m.role === b[i].role);
}

export function blankProfile(agent: string): AgentProfile {
  return { agent, nickname: null, color: null, accessory: null, catchphrase: null, updatedAt: null };
}

/** 档案里起了名字就用名字，否则用默认名。 */
export function displayName(profile: AgentProfile | undefined, fallback: string): string {
  return profile?.nickname?.trim() || fallback;
}

/** 前端先挡一遍，提示和后端一致；后端仍会再校验。 */
export function profileProblem(p: AgentProfile): string | null {
  if (p.nickname && [...p.nickname.trim()].length > MAX_NICKNAME) return `名字最多 ${MAX_NICKNAME} 个字`;
  if (p.catchphrase && [...p.catchphrase.trim()].length > MAX_CATCHPHRASE) return `口头禅最多 ${MAX_CATCHPHRASE} 个字`;
  if (p.color && !isHex(p.color)) return "颜色要写成 #rrggbb";
  return null;
}
