import { useMatch } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { ShieldX } from "lucide-react";
import { useProjectSettings } from "@/hooks/useSettings";

/**
 * 旧的永久「完全放权」不再受支持 (06 §9 第 2 条)。若检测到升级前遗留的 fullAccess，
 * 顶部提示该设置已失效、需要在项目设置里重新确认改用受限沙箱，而不再宣称「防护已关闭」。
 */
export function FullAccessBanner() {
  const listMatch = useMatch("/p/:projectId");
  const detailMatch = useMatch("/p/:projectId/t/:taskId");
  const projectId = listMatch?.params.projectId ?? detailMatch?.params.projectId;
  const settings = useProjectSettings(projectId ?? undefined);
  const legacy = !!projectId && !!settings.data?.fullAccess;

  return (
    <AnimatePresence>
      {legacy && (
        <motion.div
          role="alert"
          initial={{ height: 0, opacity: 0 }}
          animate={{ height: "auto", opacity: 1 }}
          exit={{ height: 0, opacity: 0 }}
          className="flex shrink-0 items-center justify-center gap-1.5 overflow-hidden border-b border-caution bg-caution-bg py-1.5 text-meta font-medium text-caution"
        >
          <ShieldX className="size-3.5" />
          检测到旧的永久完全放权设置，已失效——请在项目设置中重新确认改用受限沙箱
        </motion.div>
      )}
    </AnimatePresence>
  );
}
