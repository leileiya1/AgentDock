# AgentFlow 桌面端前端（apps/desktop/src）

按《02-桌面端UI实现方案 v1.1》实现的全部前端。技术栈：Vite + React 18 + TypeScript + **Tailwind CSS v4 + shadcn/ui（Radix）+ Motion** + TanStack Query + Zustand + React Router + Monaco（懒加载）。

样式全部走 Tailwind，无手写 CSS 文件（已移除 tokens/global/components.css）；设计 token 集中在 `src/styles/theme.css` 的 `@theme` 里，shadcn 变量桥接同文件。shadcn 基础组件在 `src/components/ui/`。琥珀=人类介入 的语义与 02 §2 一致，Motion 只做克制的深度/微交互（面板滑入、标签下划线、审批栏、toast），`prefers-reduced-motion` 全退化。

## 开发 / 构建

```bash
cd apps/desktop
bun install
bun run dev        # Vite dev server → http://127.0.0.1:1420
bun run typecheck  # tsc --noEmit
bun run build      # tsc + vite build → dist/
```

后端交互只经 `src/generated/bindings.ts`（tauri-specta 生成物，勿手改）。

## 需要 Codex 在 `src-tauri` 侧补的两处接线（本方不得改 src-tauri）

`apps/desktop/src-tauri/tauri.conf.json` 当前 `build.frontendDist = "../src"`，且没有 `devUrl`。要让 `tauri dev/build` 正常工作，需改为：

```jsonc
"build": {
  "frontendDist": "../dist",          // Vite 产物目录
  "devUrl": "http://127.0.0.1:1420",  // 对齐 vite.config.ts 的固定端口
  "beforeDevCommand": "bun run dev",
  "beforeBuildCommand": "bun run build"
}
```

## 实现说明与相对规格的两处偏差

1. **事件通道**：`generated/bindings.ts` 目前只导出了 `commands`，没有 tauri-specta 生成的 `events` 助手。契约（03 §4）里事件是齐全的，因此前端在 `src/lib/tauriEvents.ts` 用 `@tauri-apps/api` 的 `listen` 建了一层**类型化桥接**（payload 类型严格对齐 03 §4），不 mock 任何命令/事件。等 `xtask gen` 补出 `events` 后，此文件可退化为再导出。已登记到 03 CHANGELOG。
2. **简中字体**：规格建议的 `@fontsource/ibm-plex-sans-sc` 在 npm 上不存在（且整包 CJK webfont 体积过大）。改为字体栈内引用系统简中字体（PingFang SC / Microsoft YaHei 等），拉丁字仍用打包的 IBM Plex Sans。满足"离线不拉外网字体"的硬约束。

## 目录

- `styles/theme.css` Tailwind v4 入口：`@theme` 设计 token + shadcn 变量 + 深色基底
- `components/ui/` shadcn/ui 基础组件（button/dialog/select/switch/tooltip/scroll-area/…）
- `copy/` 状态/阻塞/错误/Agent 的中文文案权威表（02 §8）
- `lib/` 命令 unwrap、事件桥接、格式化、diff 解析、Monaco 本地化、query key
- `stores/` Zustand：UI 状态、日志环形缓冲、toast
- `hooks/` TanStack Query 封装 + 事件驱动失效 + 日志流
- `components/` StateBadge / AgentMark / Timeline / RunLogViewer / DiffPanel / IssueCard / ApprovalBar / …
- `routes/` Onboarding / TaskList / TaskDetail(+四标签) / Settings / NewTaskDialog
