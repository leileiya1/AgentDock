import { Button } from "@/components/ui/button";
import { PixelBot } from "@/components/pixel/PixelBot";
import { PixelIcon } from "@/components/pixel/PixelIcon";

export function HomeEmpty({ onNew }: { onNew: () => void }) {
  return (
    <section className="flex flex-col items-center rounded-card border border-line bg-raised px-4 py-10 text-center" aria-labelledby="home-empty-title">
      <div className="relative flex items-end gap-1.5" aria-hidden>
        <PixelBot member="claude_code" mode="idle" size={44} decorative />
        <PixelBot member="codex" mode="idle" size={44} decorative />
        <span className="absolute -top-1 left-[58%] animate-px-zz font-pixel text-meta text-t3">z</span>
        <span className="absolute -top-2 left-[66%] animate-px-zz font-pixel text-body text-t3 [animation-delay:1.2s]">Z</span>
      </div>
      <h2 id="home-empty-title" className="mt-4 text-section font-bold text-t1">还没有任务</h2>
      <p className="mx-auto mt-1.5 max-w-md text-body leading-relaxed text-t2">
        小队在打盹。派一个任务，它们会接力完成计划、开发、验证和审查，最后交给你拍板。
      </p>
      <Button variant="primary" onClick={onNew} className="mt-4" title="新建任务 (⌘N)">
        <PixelIcon name="plus" size={11} /> 新建任务
      </Button>
    </section>
  );
}
