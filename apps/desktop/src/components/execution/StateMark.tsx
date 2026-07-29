import {
  CircleCheck,
  CircleDashed,
  CircleX,
  ClipboardList,
  CodeXml,
  GitMerge,
  Info,
  LoaderCircle,
  Scale,
  ShieldCheck,
  TriangleAlert,
  UserCheck,
  type LucideIcon,
} from "lucide-react";
import type { NodeState, Phase } from "@/copy/events";
import { cn } from "@/lib/utils";

/**
 * 执行结果通道 (05 §3.2). 颜色不是唯一信号——每个状态同时给出图标和文字，
 * 所以黑白截图和色弱模式下仍然可区分。
 */
const STATE: Record<NodeState, { icon: LucideIcon; label: string; text: string; spin?: boolean; pulse?: boolean }> = {
  pending: { icon: CircleDashed, label: "未开始", text: "text-t3" },
  running: { icon: LoaderCircle, label: "进行中", text: "text-run", spin: true },
  ok: { icon: CircleCheck, label: "通过", text: "text-ok" },
  attention: { icon: TriangleAlert, label: "等待你", text: "text-human" },
  failed: { icon: CircleX, label: "失败", text: "text-bad" },
  info: { icon: Info, label: "已记录", text: "text-t2" },
};

export function stateLabel(state: NodeState): string {
  return STATE[state].label;
}

export function stateTextClass(state: NodeState): string {
  return STATE[state].text;
}

interface StateMarkProps {
  state: NodeState;
  /** 覆盖默认状态词，例如 run 用「成功 / 已中断」。 */
  label?: string;
  /** 仅图标（行内已有文字时）。 */
  iconOnly?: boolean;
  className?: string;
}

export function StateMark({ state, label, iconOnly, className }: StateMarkProps) {
  const meta = STATE[state];
  const Icon = meta.icon;
  const text = label ?? meta.label;
  return (
    <span className={cn("inline-flex shrink-0 items-center gap-1", meta.text, className)} title={text}>
      <Icon className={cn("size-4", meta.spin && "animate-spin")} aria-hidden />
      {iconOnly ? <span className="sr-only">{text}</span> : <span className="text-[12px] font-medium">{text}</span>}
    </span>
  );
}

/**
 * 业务阶段通道 (05 §3.2). 验证用盾牌，绝不伪装成某个 AI Provider。
 */
const PHASE_ICON: Record<Phase, LucideIcon> = {
  plan: ClipboardList,
  develop: CodeXml,
  validate: ShieldCheck,
  review: Scale,
  approval: UserCheck,
  delivery: GitMerge,
};

export function PhaseIcon({ phase, className }: { phase: Phase; className?: string }) {
  const Icon = PHASE_ICON[phase];
  return <Icon className={cn("size-4 shrink-0 text-t2", className)} aria-hidden />;
}
