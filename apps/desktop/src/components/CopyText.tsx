import type { ReactNode } from "react";
import { copyText } from "@/lib/format";
import { toast } from "@/stores/toastStore";
import { cn } from "@/lib/utils";

interface Props {
  value: string;
  children?: ReactNode;
  className?: string;
  title?: string;
  mono?: boolean;
}

/** Click-to-copy inline text (SHA / branch / path). Emits a "已复制" toast. */
export function CopyText({ value, children, className, title, mono = true }: Props) {
  const onCopy = async () => {
    const ok = await copyText(value);
    toast.info(ok ? "已复制" : "复制失败");
  };
  return (
    <button
      type="button"
      onClick={onCopy}
      title={title ?? `点击复制：${value}`}
      className={cn(
        "border-b border-dashed border-transparent transition-colors hover:border-t3",
        mono && "font-mono",
        className
      )}
    >
      {children ?? value}
    </button>
  );
}
