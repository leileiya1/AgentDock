import { Copy, FileQuestion } from "lucide-react";
import type { ResultCard as ResultCardModel } from "@/lib/logs/resultCard";
import { copyText } from "@/lib/format";
import { toast } from "@/stores/toastStore";
import { Button } from "@/components/ui/button";

/**
 * 「本次结果」结果卡 (05 §4.1): 一句话结论 + 完成了什么 / 验证 / 仍需注意 / 下一步。
 * 内容全部来自 `lib/logs` 的纯函数，这里只负责排版。
 */
export function ResultCard({
  card,
  title,
  onOpenTechnical,
}: {
  card: ResultCardModel | null;
  title: string;
  onOpenTechnical: () => void;
}) {
  if (!card) {
    // 提取失败时给出说明和出口，绝不把原始 JSON 塞回主要内容 (05 §4.2)。
    return (
      <section className="rounded-[var(--radius-panel)] border border-line bg-panel/60 p-4">
        <div className="flex items-center gap-2 text-t2">
          <FileQuestion className="size-4 shrink-0" aria-hidden />
          <span className="text-[13px]">未能提取可读总结</span>
        </div>
        <p className="mt-1.5 text-[12px] text-t3">
          这次运行没有留下结构化结果或可读文字。原始输出仍然完整保存在技术详情里。
        </p>
        <Button variant="outline" size="sm" className="mt-3" onClick={onOpenTechnical}>
          查看技术详情
        </Button>
      </section>
    );
  }

  const plainText = [
    card.conclusion,
    section("完成了什么", card.completed),
    section("验证", card.validation),
    section("仍需注意", card.concerns),
    card.nextAction ? `下一步\n${card.nextAction}` : "",
  ]
    .filter(Boolean)
    .join("\n\n");

  return (
    <section className="rounded-[var(--radius-panel)] border border-line bg-panel/60 p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="text-[13px] font-semibold text-t2">{title}</h2>
          <p className="mt-1 whitespace-pre-wrap text-[14px] leading-relaxed text-t1">{card.conclusion}</p>
        </div>
        <Button
          variant="ghost"
          size="sm"
          className="shrink-0"
          onClick={async () => {
            toast.info((await copyText(plainText)) ? "总结已复制" : "复制失败");
          }}
        >
          <Copy className="size-3.5" /> 复制总结
        </Button>
      </div>

      <Group label="完成了什么" items={card.completed} />
      <Group label="验证" items={card.validation} />
      <Group label="仍需注意" items={card.concerns} tone="attention" />

      {card.nextAction && (
        <div className="mt-3 border-t border-line/70 pt-3">
          <h3 className="text-[12px] font-semibold text-t2">下一步</h3>
          <p className="mt-1 text-[13px] text-t1">{card.nextAction}</p>
        </div>
      )}

      {!card.structured && (
        <p className="mt-3 text-[11px] text-t3">
          这次运行没有返回结构化结果，以上是 Agent 最后一段自然语言总结。
        </p>
      )}
    </section>
  );
}

function Group({ label, items, tone }: { label: string; items: string[]; tone?: "attention" }) {
  if (items.length === 0) return null;
  return (
    <div className="mt-3">
      <h3 className={`text-[12px] font-semibold ${tone === "attention" ? "text-human" : "text-t2"}`}>{label}</h3>
      <ul className="mt-1 list-none space-y-1">
        {items.map((item, index) => (
          <li key={`${label}-${index}`} className="flex gap-2 text-[13px] leading-relaxed text-t1">
            <span className="text-t3" aria-hidden>•</span>
            <span className="min-w-0">{item}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

const section = (label: string, items: string[]) =>
  items.length > 0 ? `${label}\n${items.map((item) => `• ${item}`).join("\n")}` : "";
