import type { ReactNode } from "react";
import { motion } from "motion/react";

interface Props {
  icon?: ReactNode;
  title: string;
  hint?: string;
  action?: ReactNode;
}

/** Restrained single-color line art + 说明发生了什么 + 下一步 (02 §6/§8). */
export function EmptyState({ icon, title, hint, action }: Props) {
  return (
    <motion.div
      className="flex flex-col items-center gap-3 px-6 py-12 text-center text-t2"
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.24, ease: [0.16, 1, 0.3, 1] }}
    >
      {icon && (
        <div className="grid size-11 place-items-center rounded-full border border-line bg-panel/60 text-t3">
          {icon}
        </div>
      )}
      <div className="text-[15px] font-medium text-t1">{title}</div>
      {hint && <p className="max-w-md text-[13px] text-t2">{hint}</p>}
      {action && <div className="mt-1">{action}</div>}
    </motion.div>
  );
}
