import { describe, expect, it } from "bun:test";
import type { AgentEvent, AgentEventKind } from "@/generated/bindings";
import { extractReadableText, looksLikeProtocol, redactNoise, toReadable, unwrapJson } from "./readable";
import { buildResultCard, liveProgress } from "./resultCard";

const line = (kind: AgentEventKind, summary: string, text: string | null = null): AgentEvent => ({
  ts: "2026-01-01T12:00:00Z",
  stream: "stdout",
  kind,
  summary,
  text,
});

/** 主视图里出现这些就是回归 (05 §4.2)。 */
const forbidden = (value: string) => [
  /^[[{]/.test(value.trim()),
  value.includes('"type":'),
  value.includes("item.completed"),
  value.includes("command_execution"),
  value.includes("/var/folders/"),
  /\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b/i.test(value),
];

describe("嵌套 JSON 解析 (05 §4.2)", () => {
  it("递归解开被二次、三次转义的 JSON 字符串", () => {
    const inner = JSON.stringify({ summary: "修复了空指针" });
    const middle = JSON.stringify({ item: { type: "agent_message", text: inner } });
    const parsed = unwrapJson(middle) as Record<string, unknown>;

    expect(extractReadableText(parsed)).toBe("修复了空指针");
  });

  it("按候选字段顺序提取：agent_message.text → aggregated_output.summary → result.summary", () => {
    expect(extractReadableText({ agent_message: { text: "甲" }, result: { summary: "乙" } })).toBe("甲");
    expect(extractReadableText({ aggregated_output: { summary: "乙" } })).toBe("乙");
    expect(extractReadableText({ result: { summary: "丙" } })).toBe("丙");
  });

  it("提取不到可读文字时返回 null，绝不回退成原始 JSON", () => {
    expect(extractReadableText({ type: "item.completed", item: { id: "x" } })).toBeNull();
    expect(extractReadableText("{\"type\":\"turn.completed\"}")).toBeNull();
  });

  it("识别 Anthropic 风格的 content 块", () => {
    expect(extractReadableText([{ type: "text", text: "第一段" }, { type: "text", text: "第二段" }]))
      .toBe("第一段\n第二段");
  });
});

describe("噪声识别与脱敏 (05 §4.2)", () => {
  it("JSON 原文与协议字段不算可读内容", () => {
    expect(looksLikeProtocol('{"type":"item.completed"}')).toBe(true);
    expect(looksLikeProtocol("item.completed")).toBe(true);
    expect(looksLikeProtocol("已修复登录空指针")).toBe(false);
  });

  it("临时路径、内部编号、校验值被替换成可读占位", () => {
    const raw =
      "结果写入 /var/folders/xy/T/agentflow-run/result.json，任务 3f6d1c2a-9b41-4c2e-8a77-2b1d5e6f7a90，" +
      "校验值 " + "a".repeat(64);
    const clean = redactNoise(raw);

    expect(clean).toContain("（临时文件）");
    expect(clean).toContain("（内部编号）");
    expect(clean).toContain("（校验值）");
    expect(forbidden(clean).some(Boolean)).toBe(false);
  });

  it("工具调用与系统行永远不进主视图", () => {
    expect(toReadable(line("tool_use", "Bash bun test --coverage")).mainView).toBe(false);
    expect(toReadable(line("system", "session start")).mainView).toBe(false);
  });
});

describe("Codex 嵌套 JSON + command execution + 最终总结 (05 §11)", () => {
  const events: AgentEvent[] = [
    line("assistant_text", JSON.stringify({ type: "thread.started", thread_id: "abc" })),
    line("assistant_text", JSON.stringify({
      type: "item.completed",
      item: { type: "agent_message", text: "先定位空指针来源。" },
    })),
    line("assistant_text", JSON.stringify({
      type: "item.completed",
      item: { type: "command_execution", command: "bun test", status: "completed" },
    })),
    line("assistant_text", JSON.stringify({
      type: "item.completed",
      item: {
        type: "agent_message",
        text: JSON.stringify({
          summary: "修复登录空指针并补充单测。",
          changes: ["src/auth/login.ts 增加可选链兜底", "新增 login.test.ts"],
          tests: ["12 项测试全部通过", "typecheck 通过"],
          issues: ["Windows rename 行为尚未在真实环境验证"],
          next_action: "等待人工批准后合并",
        }),
      },
    })),
    line("assistant_text", JSON.stringify({ type: "turn.completed" })),
  ];

  it("主视图里没有 JSON、协议字段、命令原文或临时路径", () => {
    for (const event of events.map(toReadable).filter((e) => e.mainView)) {
      expect(forbidden(event.text!).some(Boolean)).toBe(false);
    }
    expect(toReadable(events[2]).mainView).toBe(false);
    expect(toReadable(events[2]).text).toBe("命令执行完成");
  });

  it("最终总结取结构化结果，而不是最后一行「本轮处理完成」", () => {
    const card = buildResultCard(events)!;

    expect(card.structured).toBe(true);
    expect(card.conclusion).toBe("修复登录空指针并补充单测。");
    expect(card.completed).toEqual(["src/auth/login.ts 增加可选链兜底", "新增 login.test.ts"]);
    expect(card.validation).toEqual(["12 项测试全部通过", "typecheck 通过"]);
    expect(card.concerns).toEqual(["Windows rename 行为尚未在真实环境验证"]);
    expect(card.nextAction).toBe("等待人工批准后合并");
  });

  it("实时进展只保留人类可读短句", () => {
    const progress = liveProgress(events);

    expect(progress.map((p) => p.text)).toContain("先定位空指针来源。");
    expect(progress.every((p) => !forbidden(p.text!).some(Boolean))).toBe(true);
  });
});

describe("退化路径 (05 §4.2 / §4.4)", () => {
  it("没有结构化结果时退回最后一段有意义的 AI 文字", () => {
    const card = buildResultCard([
      line("assistant_text", "正在检查测试。"),
      line("assistant_text", "已经补上缺失的断言。"),
      line("result", "本轮处理完成"),
    ])!;

    expect(card.structured).toBe(false);
    expect(card.conclusion).toBe("已经补上缺失的断言。");
  });

  it("完全无法提取时返回 null，由 UI 提示而不是泄漏原文", () => {
    const card = buildResultCard([
      line("raw", '{"type":"item.completed","item":{"id":"x"}}'),
      line("tool_use", "Bash rm -rf /tmp/x"),
    ]);

    expect(card).toBeNull();
  });

  it("历史 Codex 行（整包 JSON 存进 summary）走同一套策略", () => {
    const legacy = line(
      "assistant_text",
      '{"type":"item.completed","item":{"type":"agent_message","text":"已完成重构。"}}'
    );
    const readable = toReadable(legacy);

    expect(readable.kind).toBe("narrative");
    expect(readable.text).toBe("已完成重构。");
    expect(readable.mainView).toBe(true);
  });

  it("API Provider 的 result.summary 形状同样被识别", () => {
    const card = buildResultCard([
      line("result", "structured", JSON.stringify({ result: { summary: "已按要求调整接口。", tests: ["3 项测试通过"] } })),
    ])!;

    expect(card.conclusion).toBe("已按要求调整接口。");
    expect(card.validation).toEqual(["3 项测试通过"]);
  });
});
