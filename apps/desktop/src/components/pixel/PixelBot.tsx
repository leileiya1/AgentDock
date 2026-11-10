import type { AgentKind } from "@/generated/bindings";
import { agentLabel } from "@/copy/agents";
import { cn } from "@/lib/utils";

/**
 * 12×12 pixel squad member (方向 B「协作工作室」).
 *
 * Every Provider is a small robot; the human reviewer is a pixel person. Robots
 * only animate when something is really happening: `work` bobs in two frames,
 * `hand` waves while waiting for you, `done` lights the chest LED green. The
 * global reduced-motion policy in theme.css stops every loop.
 */

export type BotFamily =
  | "claude"
  | "codex"
  | "qoder"
  | "gemini"
  | "qwen"
  | "grok"
  | "deepseek"
  | "kimi"
  | "minimax"
  | "runner"
  | "neutral"
  | "you";

export type BotMode = "idle" | "work" | "hand" | "done";

/** Who appears on screen: a Provider, the local validator, or you. */
export type SquadMember = AgentKind | "runner" | "you";

type Role = "ink" | "body" | "light" | "eye" | "led" | "skin" | "hair" | "mouth";
type Px = readonly [x: number, y: number, w: number, h: number, role: Role];

type Antenna = "single" | "twin" | "crown" | "helmet" | "bent";
type Eyes = "tall" | "visor" | "big" | "dot";

const SHAPE: Record<Exclude<BotFamily, "you">, { antenna: Antenna; eyes: Eyes; round?: boolean }> = {
  claude: { antenna: "single", eyes: "tall" },
  codex: { antenna: "twin", eyes: "visor" },
  qoder: { antenna: "crown", eyes: "big", round: true },
  gemini: { antenna: "single", eyes: "big", round: true },
  qwen: { antenna: "crown", eyes: "tall" },
  grok: { antenna: "bent", eyes: "dot" },
  deepseek: { antenna: "twin", eyes: "tall" },
  kimi: { antenna: "bent", eyes: "big" },
  minimax: { antenna: "helmet", eyes: "tall" },
  runner: { antenna: "helmet", eyes: "dot" },
  neutral: { antenna: "helmet", eyes: "dot" },
};

const FAMILY_BY_AGENT: Record<string, BotFamily> = {
  claude_code: "claude",
  anthropic_api: "claude",
  codex: "codex",
  openai_api: "codex",
  qoder_cli: "qoder",
  gemini_cli: "gemini",
  qwen_code: "qwen",
  grok_cli: "grok",
  grok_api: "grok",
  deepseek_api: "deepseek",
  kimi_api: "kimi",
  minimax_api: "minimax",
};

export function botFamily(member: SquadMember): BotFamily {
  if (member === "you") return "you";
  if (member === "runner") return "runner";
  const exact = FAMILY_BY_AGENT[member];
  if (exact) return exact;
  // External or legacy Provider ids ("claude", "openai-compatible", …) still get
  // their vendor's robot when the vendor is recognizable from the id.
  const id = member.toLowerCase();
  const hit = FAMILY_HINTS.find(([needle]) => id.includes(needle));
  return hit ? hit[1] : "neutral";
}

const FAMILY_HINTS: Array<[string, BotFamily]> = [
  ["claude", "claude"],
  ["anthropic", "claude"],
  ["codex", "codex"],
  ["openai", "codex"],
  ["qoder", "qoder"],
  ["gemini", "gemini"],
  ["qwen", "qwen"],
  ["grok", "grok"],
  ["deepseek", "deepseek"],
  ["kimi", "kimi"],
  ["minimax", "minimax"],
];

export function memberLabel(member: SquadMember): string {
  if (member === "you") return "你";
  if (member === "runner") return "验证机";
  return agentLabel(member);
}

/** Short name for tight spots such as the squad strip ("Claude", "Qoder"). */
export function memberShortLabel(member: SquadMember): string {
  return memberLabel(member).replace(/\s+(Code|CLI|API)$/, "");
}

/** Station / avatar background tint for a squad member. */
export function botTint(member: SquadMember): string {
  return `var(--color-bot-${botFamily(member)}-tint)`;
}

const ANTENNA: Record<Antenna, Px[]> = {
  single: [[5, 0, 2, 1, "body"], [5, 1, 2, 1, "ink"]],
  twin: [[2, 0, 1, 1, "body"], [9, 0, 1, 1, "body"], [2, 1, 1, 1, "ink"], [9, 1, 1, 1, "ink"]],
  crown: [[4, 0, 4, 1, "body"], [5, 1, 2, 1, "ink"]],
  helmet: [[3, 1, 6, 1, "ink"]],
  bent: [[9, 0, 1, 1, "body"], [8, 1, 1, 1, "ink"]],
};

function eyes(kind: Eyes, mode: BotMode): Px[] {
  if (mode === "done") return [[3, 5, 1, 1, "eye"], [4, 4, 1, 1, "eye"], [7, 4, 1, 1, "eye"], [8, 5, 1, 1, "eye"]];
  if (mode === "idle") return [[3, 5, 2, 1, "eye"], [7, 5, 2, 1, "eye"]];
  if (kind === "visor") return [];
  if (kind === "big") return [[3, 4, 2, 2, "eye"], [7, 4, 2, 2, "eye"]];
  if (kind === "dot") return [[4, 5, 1, 1, "eye"], [7, 5, 1, 1, "eye"]];
  return [[4, 4, 1, 2, "eye"], [7, 4, 1, 2, "eye"]];
}

function robot(family: Exclude<BotFamily, "you">): { base: Px[]; visor: boolean } {
  const shape = SHAPE[family];
  const head: Px[] = shape.round
    ? [[2, 2, 8, 6, "ink"], [1, 3, 10, 4, "ink"]]
    : [[1, 2, 10, 6, "ink"], [0, 4, 1, 2, "ink"], [11, 4, 1, 2, "ink"]];
  const helmetGlow: Px[] = shape.antenna === "helmet" ? [[3, 2, 6, 1, "light"]] : [];
  return {
    visor: shape.eyes === "visor",
    base: [
      ...ANTENNA[shape.antenna],
      ...head,
      [2, 3, 8, 4, "body"],
      ...helmetGlow,
      [3, 4, 6, 2, "ink"],
      [3, 8, 6, 3, "ink"],
      [4, 8, 4, 2, "light"],
      [3, 11, 2, 1, "ink"],
      [7, 11, 2, 1, "ink"],
    ],
  };
}

const HUMAN: Px[] = [
  [3, 1, 6, 1, "ink"],
  [2, 2, 8, 6, "ink"],
  [3, 3, 6, 4, "skin"],
  [3, 2, 6, 1, "hair"],
  [3, 3, 1, 1, "hair"],
  [8, 3, 1, 1, "hair"],
  [3, 8, 6, 3, "ink"],
  [4, 8, 4, 2, "body"],
  [3, 11, 2, 1, "ink"],
  [7, 11, 2, 1, "ink"],
];

function humanFace(mode: BotMode): Px[] {
  if (mode === "done") return [[3, 4, 1, 1, "ink"], [4, 3, 1, 1, "ink"], [7, 3, 1, 1, "ink"], [8, 4, 1, 1, "ink"], [4, 5, 4, 1, "mouth"]];
  if (mode === "idle") return [[4, 5, 1, 1, "ink"], [7, 5, 1, 1, "ink"]];
  return [[4, 4, 1, 1, "ink"], [7, 4, 1, 1, "ink"], [5, 6, 2, 1, "mouth"]];
}

function fill(role: Role, family: BotFamily): string {
  switch (role) {
    case "ink":
      return "var(--color-bot-ink)";
    case "eye":
      return "var(--color-bot-eye)";
    case "led":
      return "var(--color-bot-led)";
    case "light":
      return family === "you" ? "var(--color-bot-you)" : `var(--color-bot-${family}-light)`;
    case "skin":
      return "var(--color-bot-you-light)";
    case "hair":
      return "var(--color-bot-you-hair)";
    case "mouth":
      return "var(--color-bot-you-mouth)";
    default:
      return `var(--color-bot-${family})`;
  }
}

function Rects({ px, family }: { px: Px[]; family: BotFamily }) {
  return (
    <>
      {px.map(([x, y, w, h, role], i) => (
        <rect key={i} x={x} y={y} width={w} height={h} style={{ fill: fill(role, family) }} />
      ))}
    </>
  );
}

export function PixelBot({
  member,
  mode = "idle",
  size = 32,
  className,
  title,
  decorative = false,
  color,
}: {
  member: SquadMember;
  mode?: BotMode;
  size?: number;
  className?: string;
  title?: string;
  /** Hide from assistive tech when a visible label already names the member. */
  decorative?: boolean;
  /** Body colour from the agent's profile (`#rrggbb`); the face plate follows it, lighter. */
  color?: string | null;
}) {
  const family = botFamily(member);
  const label = title ?? `${memberLabel(member)}${MODE_LABEL[mode] ? ` · ${MODE_LABEL[mode]}` : ""}`;
  const isHuman = family === "you";
  const shape = isHuman ? null : robot(family);

  return (
    <span
      className={cn("inline-block shrink-0 align-middle", mode === "work" && "animate-px-bob", mode === "idle" && "opacity-60", className)}
      style={{
        width: size,
        height: size,
        ...(color && family !== "you"
          ? { [`--color-bot-${family}`]: color, [`--color-bot-${family}-light`]: `color-mix(in srgb, ${color} 38%, white)` }
          : {}),
      }}
      role={decorative ? undefined : "img"}
      aria-label={decorative ? undefined : label}
      aria-hidden={decorative || undefined}
      title={decorative ? undefined : label}
    >
      <svg viewBox="0 0 12 12" width={size} height={size} className="pixelated block" aria-hidden>
        {isHuman ? (
          <>
            <Rects px={HUMAN} family={family} />
            <Rects px={humanFace(mode)} family={family} />
          </>
        ) : (
          <>
            <Rects px={shape!.base} family={family} />
            <Rects px={eyes(SHAPE[family as Exclude<BotFamily, "you">].eyes, mode)} family={family} />
            {shape!.visor && mode !== "done" && mode !== "idle" && (
              <rect
                x={3}
                y={4}
                width={3}
                height={1}
                className={cn(mode === "work" && "animate-px-scan")}
                style={{ fill: "var(--color-bot-eye)" }}
              />
            )}
            <rect
              x={5}
              y={9}
              width={2}
              height={1}
              className={cn(mode === "work" && "animate-px-blink")}
              style={{
                fill: mode === "done" ? "var(--color-bot-led)" : mode === "work" ? "var(--color-status-running)" : `var(--color-bot-${family})`,
              }}
            />
          </>
        )}
        {mode === "hand" && (
          <g className="animate-px-wave">
            <rect x={9} y={8} width={1} height={1} style={{ fill: "var(--color-bot-ink)" }} />
            <rect x={10} y={7} width={1} height={1} style={{ fill: "var(--color-bot-ink)" }} />
            <rect x={10} y={6} width={2} height={1} style={{ fill: "var(--color-bot-you)" }} />
          </g>
        )}
      </svg>
    </span>
  );
}

const MODE_LABEL: Record<BotMode, string> = {
  idle: "待命",
  work: "工作中",
  hand: "等你处理",
  done: "完成",
};
