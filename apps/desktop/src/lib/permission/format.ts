/**
 * 权限请求的网络与命令展示解析 (06 §9 第 6/8 条). 纯函数，便于测试。
 */

export interface NetworkEndpoint {
  host: string;
  /** 端口；域名未附带端口时为 null。 */
  port: number | null;
  /** 由端口推断的协议标签（https/http/ssh/tcp 等），用于「精确协议」展示。 */
  protocol: string;
  /** 原始展示串，例如 registry.npmjs.org:443。 */
  raw: string;
}

const PORT_PROTOCOL: Record<number, string> = {
  80: "http",
  443: "https",
  22: "ssh",
  21: "ftp",
  53: "dns",
  5432: "postgres",
  3306: "mysql",
  6379: "redis",
};

/**
 * 解析后端规范化域名串为「域名 + 端口 + 协议」(§9 第 8 条)。后端保证域名已规范化，
 * 这里只负责拆分端口并推断协议标签；无法识别的端口标为 tcp。
 */
export function parseEndpoint(raw: string): NetworkEndpoint {
  const trimmed = raw.trim();
  // 保护 IPv6 字面量 [::1]:443 的方括号写法。
  const match = /^(\[[^\]]+\]|[^:]+)(?::(\d+))?$/.exec(trimmed);
  const host = match ? match[1] : trimmed;
  const port = match && match[2] ? Number(match[2]) : null;
  const protocol = port != null ? (PORT_PROTOCOL[port] ?? "tcp") : "未指定";
  return { host, port, protocol, raw: trimmed };
}

/**
 * 命令的一句话人类说明 (§9 第 6 条)。等宽 argv 由组件负责展示，这里给旁边的简短描述：
 * 取可执行文件名 + 是否带子命令，避免用户只面对一长串参数。
 */
export function argvHumanHint(argv: string[]): string {
  if (argv.length === 0) return "无命令";
  const bin = argv[0].split(/[/\\]/).pop() || argv[0];
  const sub = argv[1] && !argv[1].startsWith("-") ? ` ${argv[1]}` : "";
  return `${bin}${sub}`;
}

/** 把 argv 数组拼成可复制的等宽命令行，对含空格的参数加引号。 */
export function argvLine(argv: string[]): string {
  return argv
    .map((arg) => (/\s/.test(arg) ? `"${arg}"` : arg))
    .join(" ");
}
