import { expect, test } from "bun:test";
import type { ProjectAppearance } from "@/generated/bindings";
import { appearancePatch, blankProfile, displayName, isEmptyPatch, profilePatch, profileProblem, sameMembers } from "./studioOptions";

const look = (over: Partial<ProjectAppearance> = {}): ProjectAppearance => ({ projectId: "p", kind: null, roof: null, sign: null, yard: [], updatedAt: null, ...over });

test("外观 patch 只带改过的字段，清空的字段传 null", () => {
  const before = look({ kind: "team", roof: "#336699" });
  expect(appearancePatch(before, before)).toEqual({});
  expect(appearancePatch(before, look({ kind: "team", roof: null, yard: ["pet"] }))).toEqual({ roof: null, yard: ["pet"] });
});

test("档案 patch：前后空白不算改动，删光文字等于清空", () => {
  const before = { ...blankProfile("codex"), nickname: "小码" };
  expect(isEmptyPatch(profilePatch(before, { ...before, nickname: " 小码 " }))).toBe(true);
  expect(profilePatch(before, { ...before, nickname: "  " })).toEqual({ nickname: null });
  expect(displayName({ ...before, nickname: null }, "Codex")).toBe("Codex");
});

test("档案校验按字符数算，中文一个字算一个", () => {
  expect(profileProblem({ ...blankProfile("codex"), nickname: "一二三四五六七八九十一二三四五六" })).toBeNull();
  expect(profileProblem({ ...blankProfile("codex"), nickname: "一二三四五六七八九十一二三四五六七" })).toContain("16");
  expect(profileProblem({ ...blankProfile("codex"), color: "blue" })).toContain("#rrggbb");
});

test("成员比较看顺序和分工", () => {
  const saved = [{ agent: "codex", role: "developer" as const, position: 0, addedAt: "" }];
  expect(sameMembers(saved, [{ agent: "codex", role: "developer" }])).toBe(true);
  expect(sameMembers(saved, [{ agent: "codex", role: "both" }])).toBe(false);
});
