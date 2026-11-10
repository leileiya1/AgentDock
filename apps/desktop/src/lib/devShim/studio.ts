import type { AgentKind, AgentProfile, ProjectAppearance, ProjectMember, ProjectMemberInput, ProjectSummary } from "@/generated/bindings";
import { withMembers } from "@/lib/studio/projectSummary";

/**
 * 浏览器预览里的小镇自定义数据：外观、项目成员、Agent 档案。存在内存里，刷新页面就回到预置值。
 * 规则只做最基本的校验，真实规则在后端 `studio_profiles.rs`。
 */

const now = () => new Date().toISOString();

// 预置：演示项目里有几栋是用户「选过」的样子，有几栋分配了成员
const DEMO_LOOKS: Record<string, Partial<ProjectAppearance>> = {
  "town-2": { kind: "team", sign: "neon", yard: ["fountain", "flag"] },
  "town-3": { kind: "personal", sign: "wood" },
  "town-6": { kind: "backend", sign: "stone" },
  "town-8": { kind: "docs", yard: ["garden", "pet"] },
};
const DEMO_MEMBERS: Record<string, ProjectMemberInput[]> = {
  "town-2": [
    { agent: "claude_code", role: "developer" },
    { agent: "codex", role: "reviewer" },
    { agent: "qoder_cli", role: "developer" },
    { agent: "gemini_cli", role: "both" },
    { agent: "kimi_cli", role: "reviewer" },
  ],
  "town-5": [{ agent: "claude_code", role: "both" }],
};
const DEMO_PROFILES: AgentProfile[] = [
  { agent: "claude_code", nickname: "小克", color: null, accessory: "headphones", catchphrase: "这个我来", updatedAt: null },
];

const looks = new Map<string, ProjectAppearance>();
const members = new Map<string, ProjectMember[]>();
const profiles = new Map<string, AgentProfile>();
let seeded = false;

function toMembers(list: readonly ProjectMemberInput[], previous: readonly ProjectMember[] = []): ProjectMember[] {
  return list.map((m, position) => ({
    agent: m.agent,
    role: m.role,
    position,
    addedAt: previous.find((p) => p.agent === m.agent)?.addedAt ?? now(),
  }));
}

function seed(demo: boolean) {
  if (seeded) return;
  seeded = true;
  if (!demo) return;
  for (const [id, look] of Object.entries(DEMO_LOOKS)) looks.set(id, { projectId: id, kind: null, roof: null, sign: null, yard: [], updatedAt: now(), ...look });
  for (const [id, list] of Object.entries(DEMO_MEMBERS)) members.set(id, toMembers(list));
  for (const p of DEMO_PROFILES) profiles.set(p.agent, p);
}

function blank(projectId: string): ProjectAppearance {
  return { projectId, kind: null, roof: null, sign: null, yard: [], updatedAt: null };
}

/** 有 key 就改，值为 null 就清空；对应后端的 patch 语义。 */
function patched<T extends object>(current: T, patch: Record<string, unknown>): T {
  const next: Record<string, unknown> = { ...(current as Record<string, unknown>) };
  for (const [k, v] of Object.entries(patch)) if (v !== undefined) next[k] = v ?? (k === "yard" ? (current as Record<string, unknown>)[k] : null);
  return next as T;
}

const HEX = /^#[0-9a-fA-F]{6}$/;

/** 加上分配成员后的项目汇总。 */
export function summariesWithMembers(rows: ProjectSummary[]): ProjectSummary[] {
  return rows.map((r) => withMembers(r, (members.get(r.projectId) ?? []).map((m) => m.agent)));
}

/** 处理小镇自定义相关命令；不是这些命令时返回 undefined。`demo` = 是否预置演示数据（视觉测试时为 false）。 */
export function handleStudio(cmd: string, args: any, projectIds: readonly string[], demo: boolean): { value: unknown } | undefined {
  seed(demo);
  switch (cmd) {
    case "project_appearance_list":
      return { value: projectIds.map((id) => looks.get(id) ?? blank(id)) };
    case "project_appearance_update": {
      const patch = args.patch ?? {};
      if (patch.roof && !HEX.test(patch.roof)) throw new Error("屋顶颜色要写成 #rrggbb 这样的六位颜色");
      const next = { ...patched(looks.get(args.projectId) ?? blank(args.projectId), patch), updatedAt: now() };
      if (next.roof) next.roof = next.roof.toLowerCase();
      looks.set(args.projectId, next);
      return { value: next };
    }
    case "project_members_get":
      return { value: members.get(args.projectId) ?? [] };
    case "project_members_set": {
      const list: ProjectMemberInput[] = args.members ?? [];
      if (list.length > 12) throw new Error("一个项目最多分配 12 位 Agent");
      if (new Set(list.map((m) => m.agent)).size !== list.length) throw new Error("有 Agent 重复分配了");
      const next = toMembers(list, members.get(args.projectId));
      members.set(args.projectId, next);
      return { value: next };
    }
    case "agent_profiles_list":
      return { value: [...profiles.values()] };
    case "agent_profile_update": {
      const agent: AgentKind = args.agent;
      const current = profiles.get(agent) ?? { agent, nickname: null, color: null, accessory: null, catchphrase: null, updatedAt: null };
      const next = { ...patched(current, args.patch ?? {}), updatedAt: now() };
      profiles.set(agent, next);
      return { value: next };
    }
    case "agent_profile_reset":
      return { value: profiles.delete(args.agent) };
    default:
      return undefined;
  }
}
