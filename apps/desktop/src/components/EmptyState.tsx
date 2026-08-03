import type { ReactNode } from "react";

interface Props {
  icon?: ReactNode;
  title: string;
  hint?: string;
  action?: ReactNode;
}

/** Restrained single-color line art + 说明发生了什么 + 下一步 (02 §6/§8). */
export function EmptyState({ icon, title, hint, action }: Props) {
  return (
    <div className="flex flex-col items-center gap-3 px-6 py-12 text-center text-t2">
      {icon && (
        <div className="grid size-11 place-items-center rounded-circle border border-line bg-panel/60 text-t3">
          {icon}
        </div>
      )}
      <div className="text-section font-medium text-t1">{title}</div>
      {hint && <p className="max-w-md text-body text-t2">{hint}</p>}
      {action && <div className="mt-1">{action}</div>}
    </div>
  );
}
