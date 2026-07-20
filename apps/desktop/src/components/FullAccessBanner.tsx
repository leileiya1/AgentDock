import { useMatch } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { ShieldOff } from "lucide-react";
import { useProjectSettings } from "@/hooks/useSettings";

/** Thin persistent amber bar shown on every page of a full-access project (02 §4.5). */
export function FullAccessBanner() {
  const listMatch = useMatch("/p/:projectId");
  const detailMatch = useMatch("/p/:projectId/t/:taskId");
  const projectId = listMatch?.params.projectId ?? detailMatch?.params.projectId;
  const settings = useProjectSettings(projectId ?? undefined);
  const on = !!projectId && !!settings.data?.fullAccess;

  return (
    <AnimatePresence>
      {on && (
        <motion.div
          role="alert"
          initial={{ height: 0, opacity: 0 }}
          animate={{ height: "auto", opacity: 1 }}
          exit={{ height: 0, opacity: 0 }}
          className="flex shrink-0 items-center justify-center gap-1.5 overflow-hidden border-b border-human bg-human-bg py-1.5 text-[12px] font-medium text-human"
        >
          <ShieldOff className="size-3.5" />
          此项目已关闭命令确认防护（完全放权模式）
        </motion.div>
      )}
    </AnimatePresence>
  );
}
