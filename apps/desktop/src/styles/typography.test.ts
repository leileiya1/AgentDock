import { describe, expect, test } from "bun:test";
import { relative } from "node:path";
import { fileURLToPath } from "node:url";

const SOURCE_DIR = fileURLToPath(new URL("../", import.meta.url));
const THEME = await Bun.file(new URL("./theme.css", import.meta.url)).text();

async function runtimeSourceFiles(): Promise<Array<{ path: string; text: string }>> {
  const files: Array<{ path: string; text: string }> = [];
  const glob = new Bun.Glob("**/*.{ts,tsx}");
  for await (const absolute of glob.scan({ cwd: SOURCE_DIR, absolute: true })) {
    if (/\.test\.[^.]+$/.test(absolute)) continue;
    files.push({ path: relative(SOURCE_DIR, absolute), text: await Bun.file(absolute).text() });
  }
  return files;
}

describe("semantic typography", () => {
  test("uses the native macOS interface stack before bundled or fallback faces", () => {
    expect(THEME).toContain(
      '--font-sans: -apple-system, BlinkMacSystemFont, "SF Pro Text", "PingFang SC"',
    );
    expect(THEME).not.toContain('@import "@fontsource/ibm-plex-sans');
  });

  test("defines the five stable interface roles", () => {
    for (const role of ["micro", "meta", "body", "section", "page"]) {
      expect(THEME).toMatch(new RegExp(`--text-${role}:\\s*\\d+px`));
      expect(THEME).toMatch(new RegExp(`--text-${role}--line-height:`));
    }
  });

  test("runtime components do not invent pixel font sizes", async () => {
    const offenders = (await runtimeSourceFiles())
      .filter((file) => /text-\[\d+(?:\.\d+)?px\]/.test(file.text))
      .map((file) => file.path);
    expect(offenders).toEqual([]);
  });
});
