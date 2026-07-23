import type { NodeDiagnosticStep, NodeDiagnosticStatus } from "@/generated/bindings";

export const NODE_STEP_LABEL: Record<NodeDiagnosticStep, string> = {
  dns: "DNS 解析",
  tcp: "TCP 端口",
  ssh_authentication: "SSH 认证",
  work_root: "工作目录",
  platform: "平台信息",
  git: "Git",
  archive_tool: "归档工具",
  toolchain: "验证工具链",
};

export const NODE_STATUS_LABEL: Record<NodeDiagnosticStatus, string> = {
  passed: "通过",
  failed: "失败",
  skipped: "未执行",
};

export const NODE_RECOVERY: Record<NodeDiagnosticStep, string> = {
  dns: "检查主机名、DNS 或直接填写可达 IP。",
  tcp: "检查 SSH 端口、防火墙、VPN 和端口映射。",
  ssh_authentication: "先在终端用相同用户和端口完成免交互 SSH 登录。",
  work_root: "改用用户可写目录，或修复目录所有者与权限。",
  platform: "确认远端支持 uname；这一项不阻止连接。",
  git: "在远端安装 Git，并确保非交互 PATH 可以找到它。",
  archive_tool: "在远端安装 tar；AgentFlow 需要它解开固定提交归档。",
  toolchain: "按项目需要安装 Bun、Node、Rust 或 Python；这一项是能力提示。",
};

export function nodeStatusCopy(status: "unknown" | "online" | "offline") {
  return status === "online" ? "在线" : status === "offline" ? "不可用" : "未检查";
}
