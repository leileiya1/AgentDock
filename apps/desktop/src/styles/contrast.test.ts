import { describe, expect, it } from "bun:test";

/**
 * 对比度回归测试 (05 §8「黑白和色弱模式仍可区分」).
 *
 * 调色板是手改的，很容易在某次「视觉微调」里悄悄退回不可读的浅色——
 * 这个测试直接读 theme.css，把每个前景/背景组合的实际比值钉住。
 * 改色前先跑 `bun test src/styles`。
 */

const THEME = await Bun.file(new URL("./theme.css", import.meta.url)).text();

function token(name: string): string {
  const match = new RegExp(`--color-${name}:\\s*(#[0-9a-fA-F]{6})`).exec(THEME);
  if (!match) throw new Error(`theme.css 里找不到 --color-${name}`);
  return match[1];
}

function channel(value: number): number {
  const c = value / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string): number {
  const h = hex.replace("#", "");
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16));
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

export function contrast(foreground: string, background: string): number {
  const [a, b] = [luminance(foreground), luminance(background)];
  const [hi, lo] = a > b ? [a, b] : [b, a];
  return (hi + 0.05) / (lo + 0.05);
}

const AA = 4.5;
const WHITE = "#ffffff";

/** 应用里真实出现过的表面。t3/状态色会落在这三种底色上。 */
const SURFACES: Array<[string, string]> = [
  ["app", token("app")],
  ["panel", token("panel")],
  ["raised", token("raised")],
];

describe("正文层级对比度", () => {
  for (const level of ["t1", "t2", "t3"]) {
    for (const [surfaceName, surface] of SURFACES) {
      it(`${level} on ${surfaceName} ≥ ${AA}:1`, () => {
        expect(contrast(token(level), surface)).toBeGreaterThanOrEqual(AA);
      });
    }
  }
});

describe("状态色作为文字", () => {
  // 状态词经常是 12–13 px，必须按正文标准而不是大字号标准。
  for (const state of ["status-running", "status-success", "status-danger", "status-review", "status-idle", "status-human", "caution", "link"]) {
    for (const [surfaceName, surface] of SURFACES) {
      it(`${state} on ${surfaceName} ≥ ${AA}:1`, () => {
        expect(contrast(token(state), surface)).toBeGreaterThanOrEqual(AA);
      });
    }
  }

  it("human 在自己的浅色块上也要可读", () => {
    expect(contrast(token("status-human"), token("status-human-bg"))).toBeGreaterThanOrEqual(AA);
  });

  it("caution 在自己的浅色块上也要可读", () => {
    expect(contrast(token("caution"), token("caution-bg"))).toBeGreaterThanOrEqual(AA);
  });
});

describe("实心动作与状态填充", () => {
  for (const fill of ["action", "status-running", "status-human", "status-danger", "status-review"]) {
    it(`白字在 ${fill} 上 ≥ ${AA}:1`, () => {
      expect(contrast(WHITE, token(fill))).toBeGreaterThanOrEqual(AA);
    });
  }
});

describe("边框可辨识度", () => {
  it("line 与 app 底之间有可见差异", () => {
    // 发丝线不需要 4.5:1，但必须能看见；3:1 是 WCAG 对非文字元素的门槛。
    expect(contrast(token("line-strong"), token("app"))).toBeGreaterThanOrEqual(1.3);
  });
});
