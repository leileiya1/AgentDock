import { describe, expect, test } from "bun:test";
import { relative } from "node:path";
import { fileURLToPath } from "node:url";

const SOURCE_DIR = fileURLToPath(new URL("../", import.meta.url));
const THEME = await Bun.file(new URL("./theme.css", import.meta.url)).text();
const FLOAT_SHADOW_ALLOWLIST = new Set([
  "components/ApprovalBar.tsx",
  "components/Dialog.tsx",
  "components/SidePanel.tsx",
  "components/Toaster.tsx",
  "components/logs/TechnicalLog.tsx",
  "components/ui/select.tsx",
  "components/ui/tooltip.tsx",
]);

async function sourceFiles(): Promise<Array<{ path: string; text: string }>> {
  const files: Array<{ path: string; text: string }> = [];
  const glob = new Bun.Glob("**/*.{ts,tsx}");
  for await (const absolute of glob.scan({ cwd: SOURCE_DIR, absolute: true })) {
    const path = relative(SOURCE_DIR, absolute);
    if (path === "styles/styleDebt.test.ts") continue;
    files.push({ path, text: await Bun.file(absolute).text() });
  }
  return files;
}

describe("surface design debt", () => {
  test("keeps literal colors in the token file or the Monaco adapter only", async () => {
    const offenders = (await sourceFiles())
      .filter((file) => !file.path.includes(".test.") && file.path !== "lib/monaco.ts" && /#[0-9a-f]{3,8}\b/i.test(file.text))
      .map((file) => file.path);
    expect(offenders).toEqual([]);
  });

  test("uses semantic radii instead of arbitrary or generic large radii", async () => {
    const offenders: string[] = [];
    const forbidden = /rounded-(?:full|md|lg|xl|2xl)|rounded-\[(?!inherit\])/;
    for (const file of await sourceFiles()) {
      if (forbidden.test(file.text)) offenders.push(file.path);
    }
    expect(offenders).toEqual([]);
  });

  test("reserves shadows for overlays and temporary elevated controls", async () => {
    const offenders: string[] = [];
    for (const file of await sourceFiles()) {
      if (/shadow-(?:inner|raised|glow)/.test(file.text)) offenders.push(file.path);
      if (file.text.includes("shadow-[var(--shadow-float)]") && !FLOAT_SHADOW_ALLOWLIST.has(file.path)) {
        offenders.push(file.path);
      }
    }
    expect(offenders).toEqual([]);
  });

  test("ordinary rows do not lift on hover", async () => {
    const offenders = (await sourceFiles())
      .filter((file) => /whileHover=\{\{[^}]*\by\s*:/.test(file.text))
      .map((file) => file.path);
    expect(offenders).toEqual([]);
  });

  test("deprecated raised panels and glow shadows stay removed", () => {
    expect(THEME).not.toContain("@utility panel-raise");
    expect(THEME).not.toContain("--shadow-raised");
    expect(THEME).not.toContain("--shadow-glow-");
  });

  test("global reduced-motion policy terminates every loop", () => {
    expect(THEME).toContain("@media (prefers-reduced-motion: reduce)");
    expect(THEME).toContain("animation-iteration-count: 1 !important");
  });

  test("page components remain below the responsibility review threshold", async () => {
    const oversized = (await sourceFiles())
      .filter((file) => file.path.startsWith("routes/") && file.text.split("\n").length > 700)
      .map((file) => file.path);
    expect(oversized).toEqual([]);
  });
});
