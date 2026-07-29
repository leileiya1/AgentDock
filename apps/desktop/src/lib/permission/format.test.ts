import { describe, expect, it } from "bun:test";
import { argvHumanHint, argvLine, parseEndpoint } from "./format";

describe("网络端点解析 (06 §9 第 8 条)", () => {
  it("拆出域名、端口与协议", () => {
    const ep = parseEndpoint("registry.npmjs.org:443");
    expect(ep.host).toBe("registry.npmjs.org");
    expect(ep.port).toBe(443);
    expect(ep.protocol).toBe("https");
  });
  it("无端口时协议标未指定", () => {
    const ep = parseEndpoint("example.com");
    expect(ep.port).toBeNull();
    expect(ep.protocol).toBe("未指定");
  });
  it("中文域名保持完整", () => {
    const ep = parseEndpoint("镜像.测试.cn:443");
    expect(ep.host).toBe("镜像.测试.cn");
    expect(ep.port).toBe(443);
  });
  it("IPv6 字面量端口不被误拆", () => {
    const ep = parseEndpoint("[::1]:6379");
    expect(ep.host).toBe("[::1]");
    expect(ep.port).toBe(6379);
    expect(ep.protocol).toBe("redis");
  });
  it("未知端口标为 tcp", () => {
    expect(parseEndpoint("host:9999").protocol).toBe("tcp");
  });
});

describe("命令展示 (06 §9 第 6 条)", () => {
  it("人类说明取可执行文件与子命令", () => {
    expect(argvHumanHint(["/usr/bin/bun", "add", "zod"])).toBe("bun add");
    expect(argvHumanHint(["ls", "-la"])).toBe("ls");
    expect(argvHumanHint([])).toBe("无命令");
  });
  it("命令行对含空格参数加引号", () => {
    expect(argvLine(["git", "commit", "-m", "fix bug"])).toBe('git commit -m "fix bug"');
  });
});
