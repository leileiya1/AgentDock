import { motion } from "motion/react";
import { ArrowRight, GitBranch, Plus, ShieldCheck, Sparkles, Users } from "lucide-react";
import { Button } from "@/components/ui/button";

/**
 * 空项目首页 (还没有任何任务时). 把原本空荡的右侧变成一个有产品感的欢迎页：
 * 中心徽标 + 一句话价值主张 + 三张能力卡（描述即可 / 多智能体协作 / 安全边界）+ 主 CTA。
 * 纯 Tailwind + Motion；环境光用 CSS 动画（受 reduced-motion 全局规则约束）。
 */
export function HomeEmpty({ onNew }: { onNew: () => void }) {
  const features = [
    {
      icon: <Sparkles className="size-5" aria-hidden />,
      title: "描述即可",
      body: "用一句话说清你想要的改动，其余的规划与实现交给 AgentFlow。",
    },
    {
      icon: <Users className="size-5" aria-hidden />,
      title: "多智能体协作",
      body: "规划、开发、审查、验证由不同 Agent 分工完成，各展所长。",
    },
    {
      icon: <ShieldCheck className="size-5" aria-hidden />,
      title: "安全边界",
      body: "越界操作会暂停并请你授权，主机文件与密钥默认隔离。",
    },
  ];

  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.45, ease: [0.16, 1, 0.3, 1] }}
      className="relative overflow-hidden rounded-[16px] border border-line/70 bg-gradient-to-br from-raised via-panel to-app px-6 py-10 shadow-[var(--shadow-raised)]"
    >
      <span className="pointer-events-none absolute -right-20 -top-24 size-72 rounded-full bg-[radial-gradient(closest-side,rgba(194,94,60,0.16),transparent)] blur-2xl" aria-hidden />
      <span className="pointer-events-none absolute -bottom-24 -left-16 size-72 rounded-full bg-[radial-gradient(closest-side,rgba(169,75,43,0.10),transparent)] blur-2xl" aria-hidden />

      <div className="relative mx-auto flex max-w-xl flex-col items-center text-center">
        {/* 中心徽标 + 呼吸光环 */}
        <div className="relative grid size-16 place-items-center">
          <span className="absolute inset-0 animate-pulse-dot rounded-2xl bg-human/15" aria-hidden />
          <span className="absolute -inset-2 rounded-[20px] bg-[radial-gradient(closest-side,rgba(169,75,43,0.18),transparent)] blur-md" aria-hidden />
          <div className="relative grid size-16 place-items-center rounded-2xl border border-human/30 bg-gradient-to-br from-human-soft to-human text-white shadow-[var(--shadow-glow-human)]">
            <Sparkles className="size-7" aria-hidden />
          </div>
        </div>

        <h2 className="mt-5 text-[22px] font-semibold tracking-tight text-t1">开启你的第一个任务</h2>
        <p className="mt-2 max-w-md text-[13px] leading-relaxed text-t2">
          告诉 AgentFlow 你想要什么——它会自动规划、开发、审查并交付，越界时再回来找你确认。
        </p>

        <motion.div whileHover={{ scale: 1.03 }} whileTap={{ scale: 0.97 }} className="mt-5">
          <Button variant="primary" size="lg" onClick={onNew} title="新建任务 (⌘N)">
            <Plus className="size-4" /> 新建任务
            <ArrowRight className="size-4" />
          </Button>
        </motion.div>

        <div className="mt-3 flex items-center gap-1.5 text-[12px] text-t3">
          <GitBranch className="size-3.5" aria-hidden /> 每个任务都在独立工作树里运行，互不干扰
        </div>
      </div>

      {/* 能力卡 */}
      <div className="relative mx-auto mt-8 grid max-w-2xl grid-cols-1 gap-3 sm:grid-cols-3">
        {features.map((f, i) => (
          <motion.div
            key={f.title}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.15 + i * 0.08, duration: 0.4, ease: [0.16, 1, 0.3, 1] }}
            whileHover={{ y: -3 }}
            className="group rounded-[12px] border border-line/70 bg-panel/70 p-4 text-left shadow-[0_1px_2px_rgba(90,68,42,0.04)] transition-colors hover:border-line-strong hover:bg-raised hover:shadow-[var(--shadow-raised)]"
          >
            <div className="grid size-9 place-items-center rounded-lg bg-human-bg text-human transition-transform group-hover:scale-110">
              {f.icon}
            </div>
            <div className="mt-2.5 text-[14px] font-semibold text-t1">{f.title}</div>
            <p className="mt-1 text-[12px] leading-relaxed text-t3">{f.body}</p>
          </motion.div>
        ))}
      </div>
    </motion.div>
  );
}
