import { Plus } from "lucide-react";
import { Button } from "@/components/ui/button";

export function HomeEmpty({ onNew }: { onNew: () => void }) {
  return (
    <section className="border-y border-line/70 px-4 py-10 text-center" aria-labelledby="home-empty-title">
      <h2 id="home-empty-title" className="text-section font-semibold text-t1">还没有任务</h2>
      <p className="mx-auto mt-1.5 max-w-md text-body leading-relaxed text-t2">
        创建一个任务后，这里会显示需要你处理、正在执行和最近完成的工作。
      </p>
      <Button variant="outline" onClick={onNew} className="mt-4" title="新建任务 (⌘N)">
        <Plus className="size-4" /> 新建任务
      </Button>
    </section>
  );
}
