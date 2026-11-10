import { expect, test } from "@playwright/test";
test("team creation config and preview cannot fake execution", async ({ page }) => {
  await page.goto("/p/p1");
  await page.getByRole("button", { name: /新建任务/ }).click();
  await page.getByRole("button", { name: "切换到多 Agent 团队执行" }).click();
  const form = page.getByTestId("team-create-form"); await expect(form).toBeVisible();
  await page.getByLabel("团队人数上限", { exact: true }).fill("5");
  await page.getByRole("button", { name: "添加席位" }).click();
  await expect(form.getByRole("button", { name: "移除" })).toHaveCount(5);
  await page.getByLabel("标题", { exact: true }).fill("实际团队测试");
  await page.getByLabel("原始目标（原样保存）").fill("实现正负值双倍接口并逐项实测");
  await page.getByLabel("必须满足的条件（每行一条）").fill("正值通过\n负值通过");
  await page.getByText("运行范围与恢复预算", { exact: true }).click();
  await page.getByLabel("允许修改路径", { exact: true }).fill("src/**\nserver/**");
  await page.getByRole("button", { name: "创建并启动团队" }).click();
  await expect(page.getByRole("alert").first()).toContainText("桌面");
  await expect(form).toBeVisible();
  await page.screenshot({ path: "test-results/team-create-config.png", fullPage: false });
});
