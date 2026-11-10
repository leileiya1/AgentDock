import type { WeeklyReport } from "@/components/weekly/weeklyModel";
import { trendText } from "@/components/weekly/weeklyModel";

export const CARD = { w: 560, h: 340 };

/** 把页面里渲染好的像素机器人 SVG 变成图片（CSS 变量颜色先解析成实际颜色）。 */
export function svgToImage(svg: SVGSVGElement): Promise<HTMLImageElement> {
  const clone = svg.cloneNode(true) as SVGSVGElement;
  const src = svg.querySelectorAll("*");
  clone.querySelectorAll("*").forEach((el, i) => {
    const fill = getComputedStyle(src[i]).fill;
    if (fill && fill !== "none") el.setAttribute("fill", fill);
    el.removeAttribute("style");
    el.removeAttribute("class");
  });
  clone.setAttribute("xmlns", "http://www.w3.org/2000/svg");
  const url = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(new XMLSerializer().serializeToString(clone))}`;
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = url;
  });
}

const fmtDay = (d: Date) => d.toLocaleDateString("zh-CN", { month: "long", day: "numeric" });

/** 像素风成绩单：标题条、四格数据、MVP、评级印章、一句评语。颜色字体都取主题 token。 */
export function drawReport(ctx: CanvasRenderingContext2D, r: WeeklyReport, opts: { mvpName: string | null; bot: HTMLImageElement | null }): void {
  const css = getComputedStyle(document.documentElement);
  const c = (name: string) => css.getPropertyValue(`--color-${name}`).trim();
  const sans = css.getPropertyValue("--font-sans").trim() || "system-ui";
  const pixel = css.getPropertyValue("--font-pixel").trim() || "monospace";
  const { w, h } = CARD;
  ctx.imageSmoothingEnabled = false;
  ctx.textBaseline = "alphabetic";

  // 纸面 + 像素描边 + 点阵
  ctx.fillStyle = c("raised");
  ctx.fillRect(0, 0, w, h);
  ctx.fillStyle = c("line");
  for (let y = 70; y < h - 8; y += 12) for (let x = 8 + ((y / 12) % 2) * 6; x < w - 8; x += 12) ctx.fillRect(x, y, 1, 1);
  ctx.fillStyle = c("line-strong");
  ctx.fillRect(0, 0, w, 4);
  ctx.fillRect(0, h - 4, w, 4);
  ctx.fillRect(0, 0, 4, h);
  ctx.fillRect(w - 4, 0, 4, h);

  // 标题条
  ctx.fillStyle = c("bot-kimi-tint");
  ctx.fillRect(4, 4, w - 8, 58);
  ctx.fillStyle = c("line-strong");
  ctx.fillRect(4, 62, w - 8, 2);
  ctx.fillRect(4, 66, w - 8, 2);
  ctx.fillStyle = c("t1");
  ctx.font = `20px ${pixel}`;
  ctx.fillText("AGENTFLOW WEEKLY", 22, 34);
  ctx.fillStyle = c("t2");
  ctx.font = `600 12px ${sans}`;
  ctx.fillText(`本周成绩单 · ${fmtDay(r.from)} – ${fmtDay(r.to)}`, 22, 52);

  // 四格数据
  const tiles: Array<{ label: string; value: string; sub: string; tone: string }> = [
    { label: "合并上线", value: String(r.merged), sub: trendText(r.merged, r.mergedPrev), tone: c("ok") },
    { label: "返工率", value: r.reworkRate === null ? "—" : `${Math.round(r.reworkRate * 100)}%`, sub: "合并前返工过的比例", tone: c("t1") },
    { label: "等你处理", value: String(r.waiting), sub: r.waiting ? "门口还排着" : "收件箱清零", tone: c("status-human") },
    { label: "出岔子", value: String(r.trouble), sub: r.trouble ? "冲突或回滚" : "一次都没有", tone: r.trouble ? c("status-danger") : c("t1") },
  ];
  tiles.forEach((t, i) => {
    const x = 22 + (i % 2) * 168, y = 84 + Math.floor(i / 2) * 92;
    ctx.fillStyle = c("app");
    ctx.fillRect(x, y, 156, 80);
    ctx.fillStyle = c("line-strong");
    ctx.fillRect(x, y + 78, 156, 2);
    ctx.fillStyle = c("t3");
    ctx.font = `700 11px ${sans}`;
    ctx.fillText(t.label, x + 12, y + 20);
    ctx.fillStyle = t.tone;
    ctx.font = `30px ${pixel}`;
    ctx.fillText(t.value, x + 12, y + 54);
    ctx.fillStyle = c("t2");
    ctx.font = `500 11px ${sans}`;
    ctx.fillText(t.sub, x + 12, y + 70);
  });

  // MVP
  const mx = 366, my = 120;
  ctx.fillStyle = c("bot-kimi-tint");
  ctx.fillRect(mx, my, 172, 148);
  ctx.fillStyle = c("bot-kimi");
  ctx.fillRect(mx, my, 172, 3);
  ctx.textAlign = "center";
  ctx.fillStyle = c("caution");
  ctx.font = `800 11px ${sans}`;
  ctx.fillText("★ 本周 MVP", mx + 86, my + 22);
  if (r.mvp && opts.bot) ctx.drawImage(opts.bot, mx + 86 - 28, my + 32, 56, 56);
  ctx.fillStyle = c("t1");
  ctx.font = `800 15px ${sans}`;
  ctx.fillText(r.mvp ? opts.mvpName ?? r.mvp.agent : "虚位以待", mx + 86, my + (r.mvp ? 108 : 70));
  ctx.fillStyle = c("t2");
  ctx.font = `500 11px ${sans}`;
  ctx.fillText(r.mvp ? `合并了 ${r.mvp.merged} 个任务` : "合并一个就能上榜", mx + 86, my + (r.mvp ? 126 : 90));
  ctx.textAlign = "left";

  // 评级印章
  ctx.save();
  ctx.translate(470, 66);
  ctx.rotate(-0.2);
  ctx.strokeStyle = c("status-danger");
  ctx.lineWidth = 4;
  ctx.beginPath();
  ctx.arc(0, 0, 36, 0, Math.PI * 2);
  ctx.stroke();
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.arc(0, 0, 30, 0, Math.PI * 2);
  ctx.stroke();
  ctx.fillStyle = c("status-danger");
  ctx.textAlign = "center";
  ctx.font = `30px ${pixel}`;
  ctx.fillText(r.grade, 0, 9);
  ctx.font = `700 9px ${sans}`;
  ctx.fillText("评级", 0, 24);
  ctx.restore();
  ctx.textAlign = "left";

  // 评语
  ctx.fillStyle = c("t1");
  ctx.font = `700 13px ${sans}`;
  ctx.fillText(`「${r.comment}」`, 18, 300);
  ctx.fillStyle = c("t3");
  ctx.font = `500 11px ${sans}`;
  ctx.fillText(r.busiest ? `最忙的街坊：${r.busiest.name}（合并 ${r.busiest.merged}）` : "这周没有街坊合并任务", 22, 320);
}
