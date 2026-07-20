import { PlugZap } from "lucide-react";
import {
  siAnthropic,
  siClaudecode,
  siDeepseek,
  siGit,
  siGooglegemini,
  siKimi,
  siMinimax,
  siQwen,
  type SimpleIcon,
} from "simple-icons/icons";
import grokLogoUrl from "@/assets/provider-icons/grok.svg";
import openAiLogoUrl from "@/assets/provider-icons/openai.svg";
import { cn } from "@/lib/utils";

const BRAND_ICONS: Record<string, SimpleIcon> = {
  git: siGit,
  claude_code: siClaudecode,
  gemini_cli: siGooglegemini,
  qwen_code: siQwen,
  kimi_cli: siKimi,
  kimi_api: siKimi,
  minimax_cli: siMinimax,
  minimax_api: siMinimax,
  anthropic_api: siAnthropic,
  deepseek_api: siDeepseek,
};

/**
 * Exact vendor SVGs, kept locally so provider cards also work offline.
 * Sources: https://openai.com/brand/ and https://x.ai/news/grok-goes-global
 */
const OFFICIAL_ASSETS: Record<string, { src: string; tileClassName: string }> = {
  codex: { src: openAiLogoUrl, tileClassName: "bg-white text-black" },
  openai_api: { src: openAiLogoUrl, tileClassName: "bg-white text-black" },
  grok_cli: { src: grokLogoUrl, tileClassName: "border-black/10 bg-black text-white" },
  grok_api: { src: grokLogoUrl, tileClassName: "border-black/10 bg-black text-white" },
};

const BRAND_COLORS: Record<string, string> = {
  git: "#F05032",
  claude_code: "#D97757",
  codex: "var(--color-t1)",
  gemini_cli: "#8E75FF",
  qwen_code: "#7C6CFF",
  grok_cli: "var(--color-t1)",
  grok_api: "var(--color-t1)",
  kimi_cli: "var(--color-t1)",
  kimi_api: "var(--color-t1)",
  minimax_cli: "#F04B5F",
  minimax_api: "#F04B5F",
  openai_api: "var(--color-t1)",
  anthropic_api: "#D97757",
  deepseek_api: "#4D6BFE",
};

interface Props {
  provider: string;
  size?: number;
  className?: string;
}

/** Compact brand tile; unknown protocol Providers receive a neutral plug mark. */
export function ProviderIcon({ provider, size = 40, className }: Props) {
  const icon = BRAND_ICONS[provider];
  const officialAsset = OFFICIAL_ASSETS[provider];
  const color = BRAND_COLORS[provider] ?? "var(--color-t2)";
  // Brand marks should fill the tile enough to remain recognizable at list density.
  const glyphSize = Math.round(size * 0.72);
  return (
    <span
      className={cn(
        "grid shrink-0 place-items-center rounded-lg border border-line bg-raised/70",
        officialAsset?.tileClassName,
        className,
      )}
      style={{ color, width: size, height: size }}
      aria-hidden="true"
    >
      {officialAsset ? (
        <img src={officialAsset.src} alt="" width={glyphSize} height={glyphSize} />
      ) : icon ? (
        <svg viewBox="0 0 24 24" className="fill-current" width={glyphSize} height={glyphSize} role="img">
          <path d={icon.path} />
        </svg>
      ) : (
        <PlugZap size={glyphSize} />
      )}
    </span>
  );
}
