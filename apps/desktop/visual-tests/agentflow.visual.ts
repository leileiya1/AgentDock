import { expect, test, type Page } from "@playwright/test";

const FIXED_NOW = new Date("2026-07-30T12:00:00+08:00");

async function openStable(page: Page, path: string) {
  await page.clock.install({ time: FIXED_NOW });
  await page.goto(path);
  await expect(page.locator("#root")).not.toBeEmpty();
  await page.waitForTimeout(650);
}

const states: Array<{ name: string; path: string }> = [
  { name: "empty-project", path: "/p/p1?visual=empty" },
  { name: "multi-task-home", path: "/p/p1" },
  { name: "attention-13", path: "/p/p1?visual=attention-13" },
  { name: "task-running", path: "/p/p1/t/t13" },
  { name: "parallel-review", path: "/p/p1/t/t15" },
  { name: "provider-fallback", path: "/p/p1/t/t17" },
  { name: "permission-request", path: "/p/p1/t/t9" },
  { name: "validation-failed", path: "/p/p1/t/t16" },
  { name: "waiting-final-approval", path: "/p/p1/t/t12" },
  { name: "merged-readonly", path: "/p/p1/t/t7" },
  { name: "settings-ready", path: "/settings#environment" },
];

for (const scenario of states) {
  test(`${scenario.name} at 1280x800`, async ({ page }) => {
    await openStable(page, scenario.path);
    if (scenario.name === "provider-fallback") {
      await page.getByRole("button", { name: "打开执行详情" }).click();
    }
    await expect(page).toHaveScreenshot(`${scenario.name}-1280x800.png`, { fullPage: false });
  });
}

test("new task basic and advanced", async ({ page }) => {
  await openStable(page, "/p/p1");
  await page.getByRole("button", { name: /新建任务/ }).click();
  await expect(page).toHaveScreenshot("new-task-basic-1280x800.png");
  await page.getByRole("button", { name: /运行与治理/ }).click();
  await expect(page).toHaveScreenshot("new-task-advanced-1280x800.png");
});

for (const viewport of [
  { width: 1024, height: 768 },
  { width: 800, height: 600 },
]) {
  test(`home layout ${viewport.width}x${viewport.height}`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await openStable(page, "/p/p1");
    await expect(page).toHaveScreenshot(`home-${viewport.width}x${viewport.height}.png`);
  });
}

test("home at 200 percent zoom", async ({ page }) => {
  await openStable(page, "/p/p1");
  await page.evaluate(() => { document.documentElement.style.zoom = "2"; });
  await page.waitForTimeout(100);
  await expect(page).toHaveScreenshot("home-1280x800-zoom-200.png");
});
