import { describe, expect, test } from "bun:test";
import { fileURLToPath } from "node:url";

const THEME = await Bun.file(new URL("./theme.css", import.meta.url)).text();
const STATE_BADGE = await Bun.file(new URL("../components/StateBadge.tsx", import.meta.url)).text();

function color(name: string): string {
  const match = new RegExp(`--color-${name}:\\s*(#[0-9a-fA-F]{6})`).exec(THEME);
  if (!match) throw new Error(`missing color token: ${name}`);
  return match[1].toLowerCase();
}

describe("semantic status tokens", () => {
  test("keeps interaction and status roles separate", () => {
    expect(color("status-running")).not.toBe(color("status-human"));
    expect(color("status-human")).not.toBe(color("status-danger"));
    expect(color("action")).not.toBe(color("status-running"));
    expect(THEME).toContain("--primary: var(--color-action)");
    expect(THEME).toContain("--ring: var(--color-focus)");
  });

  test("defines every canonical workflow state", () => {
    for (const token of ["status-running", "status-human", "status-success", "status-danger", "status-review", "status-idle"]) {
      expect(color(token)).toMatch(/^#[0-9a-f]{6}$/);
    }
  });

  test("state badges use semantic washes without a hard-coded amber glow", () => {
    expect(STATE_BADGE).toContain("text-status-running");
    expect(STATE_BADGE).toContain("text-status-human");
    expect(STATE_BADGE).not.toContain("rgba(232,163,61");
    expect(STATE_BADGE).not.toContain("shadow-[0_0_16px");
  });

  test("source code does not reuse legacy run/human utilities", async () => {
    const sourceDir = fileURLToPath(new URL("../", import.meta.url));
    const glob = new Bun.Glob("**/*.{ts,tsx}");
    const offenders: string[] = [];
    const legacy = /(?:text|bg|border|ring)-(?:run|human)(?:\b|\/)|var\(--color-(?:run|human)\)/;

    for await (const path of glob.scan({ cwd: sourceDir, absolute: true })) {
      if (path.endsWith("statusTokens.test.ts")) continue;
      if (legacy.test(await Bun.file(path).text())) offenders.push(path.replace(`${sourceDir}/`, ""));
    }

    expect(offenders).toEqual([]);
  });
});
