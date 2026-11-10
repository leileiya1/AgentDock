import { expect, test } from "bun:test";
import { botFamily, memberShortLabel } from "./PixelBot";

test("maps known, legacy and unknown Provider ids to a robot family", () => {
  expect(botFamily("claude_code")).toBe("claude");
  expect(botFamily("claude")).toBe("claude");
  expect(botFamily("openai_api")).toBe("codex");
  expect(botFamily("qoder_cli")).toBe("qoder");
  expect(botFamily("acme-sidecar")).toBe("neutral");
  expect(botFamily("runner")).toBe("runner");
  expect(botFamily("you")).toBe("you");
});

test("short labels drop the tool suffix for tight layouts", () => {
  expect(memberShortLabel("claude_code")).toBe("Claude");
  expect(memberShortLabel("qoder_cli")).toBe("Qoder");
  expect(memberShortLabel("you")).toBe("你");
});
