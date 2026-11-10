import { useEffect, useState } from "react";
import { Dialog } from "@/components/Dialog";
import { SHORTCUTS, shouldIgnoreKey } from "@/lib/keys";

/** 按 ? 打开的快捷键速查表；⌘K 里也能找到。 */
export function ShortcutSheet() {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "?" && !shouldIgnoreKey(e)) {
        e.preventDefault();
        setOpen(true);
      }
    };
    const onOpen = () => setOpen(true);
    window.addEventListener("keydown", onKey);
    window.addEventListener("agentflow:shortcuts", onOpen);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("agentflow:shortcuts", onOpen);
    };
  }, []);

  return (
    <Dialog open={open} onClose={() => setOpen(false)} title="键盘快捷键" subtitle="手不离键盘，也能把活派完" width={520}>
      <div className="grid gap-4 sm:grid-cols-2">
        {SHORTCUTS.map((g) => (
          <section key={g.group} className="flex flex-col gap-1.5">
            <h3 className="text-micro font-extrabold tracking-wider text-t3">{g.group}</h3>
            {g.items.map((s) => (
              <div key={s.label} className="flex items-center justify-between gap-3 text-body text-t1">
                <span>{s.label}</span>
                <span className="flex shrink-0 gap-1">
                  {s.keys.map((k) => (
                    <kbd key={k} className="grid h-6 min-w-6 place-items-center rounded-row bg-app px-1.5 font-pixel text-micro text-t2 shadow-press ring-1 ring-line">
                      {k}
                    </kbd>
                  ))}
                </span>
              </div>
            ))}
          </section>
        ))}
      </div>
    </Dialog>
  );
}
