import { expect, test } from "bun:test";
import type { TaskSummary } from "@/generated/bindings";
import { badges, dailyPaper, mergeStreak } from "./paperModel";

const NOW = new Date(2026, 6, 30, 12);
const at = (daysAgo: number, hour = 10) => new Date(2026, 6, 30 - daysAgo, hour).toISOString();
const t = (seq: number, status: TaskSummary["status"], updatedAt: string, dev = "claude_code"): TaskSummary => ({
  id: `t${seq}`, projectId: "p", seq, title: `任务${seq}`, status, blockedReason: null, currentRevision: 1,
  developerAgent: dev, reviewerAgent: "codex", updatedAt,
});

test("小报：天气反映健康度，头条来自真实任务，本周之星按 7 天合并数", () => {
  const tasks = [t(1, "MERGED", at(0)), t(2, "MERGED", at(1), "codex"), t(3, "MERGED", at(2), "codex"), t(4, "WAITING_FOR_HUMAN_APPROVAL", at(0, 9)), t(5, "DEVELOPING", at(0, 11))];
  const p = dailyPaper(tasks, NOW);
  expect(p.weather).toBe("cloudy");
  expect(p.headlines[0].text).toBe("今天已合并 1 个任务");
  expect(p.headlines[1]).toMatchObject({ taskId: "t4", tone: "human" });
  expect(p.headlines[1].text).toContain("已经等你 3 小时");
  expect(p.star).toEqual({ agent: "codex", merged: 2 });
  expect(dailyPaper(tasks, NOW, 2).weather).toBe("rainy");
  expect(dailyPaper([t(1, "MERGED", at(0))], NOW).weather).toBe("sunny");
});

test("成就：连续合并天数、收件箱清零、夜猫子", () => {
  const tasks = [t(1, "MERGED", at(0)), t(2, "MERGED", at(1)), t(3, "MERGED", at(2, 3))];
  expect(mergeStreak(tasks, NOW)).toBe(3);
  const wall = Object.fromEntries(badges(tasks, NOW).map((b) => [b.key, b]));
  expect(wall.first.earned && wall.streak.earned && wall.inbox.earned && wall.owl.earned).toBe(true);
  expect(wall.ten).toMatchObject({ earned: false, hint: "累计合并 3 / 10" });
  expect(mergeStreak([t(1, "MERGED", at(3))], NOW)).toBe(0);
});
