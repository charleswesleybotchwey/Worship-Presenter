import { expect, test } from "@playwright/test";

// The operator's essentials, in the browser preview (the app's own stand-in data).

test("a song goes live and the switches bring the words back", async ({ page, context }) => {
  const out = await context.newPage();
  await out.goto("/#/output/main");
  await page.goto("/#/control");
  await page.locator(".lt-row--songs").first().click();
  await page.locator(".wp-btn--primary", { hasText: "Go live" }).click();
  await expect(out.locator(".tl-box").first()).toBeVisible();
  const clear = page.locator(".wp-service-actions button", { hasText: "Clear" });
  await clear.click();
  await expect(clear).toHaveAttribute("aria-pressed", "true");
  await expect(out.locator(".tl-box")).toHaveCount(0);
  await clear.click();
  await expect(out.locator(".tl-box").first()).toBeVisible();
});

test("the Transcript panel opens and listening starts without a crash", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/#/control");
  await page.locator("button[title^='The live transcript']").click();
  await page.locator(".trp button", { hasText: "Start transcribing" }).click();
  await expect(page.locator(".trp__line").first()).toBeVisible();
  expect(errors).toEqual([]);
});

for (const [w, h] of [
  [910, 512],
  [1024, 600],
  [1280, 720],
]) {
  test(`live buttons stay on screen at ${w}×${h}`, async ({ page }) => {
    await page.setViewportSize({ width: w, height: h });
    await page.goto("/#/control");
    for (const name of ["Go live", "Blank", "Logo", "Clear"]) {
      const b = page.locator("button", { hasText: name }).first();
      await expect(b).toBeInViewport({ ratio: 1 });
    }
  });
}

test("a verse sends its whole chapter to Preview, on that verse", async ({ page }) => {
  await page.goto("/#/control");
  await page.locator(".lib-tabs", { hasText: "Scriptures" }).getByText("Scriptures").click();
  await page.locator(".lt-row--verses", { hasText: "3:16" }).click();
  await expect(page.locator(".ew-pane[aria-label=Preview] .sl-slide")).toHaveCount(21);
  await expect(page.locator(".ew-pane[aria-label=Preview] .sl-slide.is-current")).toContainText("For God so loved");
});

test("several songs selected go to the schedule together", async ({ page }) => {
  await page.goto("/#/control");
  const rows = page.locator(".lt-row--songs");
  await rows.nth(0).click();
  await rows.nth(1).click({ modifiers: ["Control"] });
  await rows.nth(1).click({ button: "right" });
  await page.locator("[data-menu-depth='0'] button", { hasText: "Add 2 songs" }).click();
  await expect(page.locator(".wp-pane--schedule li")).toHaveCount(2);
});

test("several themes can be moved and deleted together (the default is kept)", async ({ page }) => {
  page.on("dialog", (d) => void d.accept());
  await page.goto("/#/control");
  await page.locator(".lib-tabs").getByText("Themes").click();
  const cards = page.locator(".lt-theme");
  for (let i = 0; i < 2; i++) {
    await cards.first().click({ button: "right" });
    await page.locator("[data-menu-depth='0'] button", { hasText: "Duplicate" }).first().click();
    // the copy opens in the theme editor: back to the service window
    await page.locator(".wp-wstab", { hasText: "Service" }).click();
    await page.locator(".lib-tabs").getByText("Themes").click();
  }
  await expect(cards).toHaveCount(3);
  // drag two onto Scripture
  await cards.nth(1).click();
  await cards.nth(2).click({ modifiers: ["Control"] });
  await cards.nth(2).dragTo(page.locator(".lt-node", { hasText: "Scripture" }));
  await expect(cards).toHaveCount(1);
  await expect(page.locator(".lt-node", { hasText: "Scripture" })).toContainText("3");
});

test("several media items and search results go to the schedule together", async ({ page }) => {
  await page.goto("/#/control");
  await page.locator(".lib-tabs").getByText("Media").click();
  const media = page.locator(".wp-row--lib");
  await media.nth(0).click();
  await media.nth(2).click({ modifiers: ["Control"] });
  await media.nth(2).click({ button: "right" });
  await page.locator("[data-menu-depth='0'] button", { hasText: "Add 2 items" }).click();
  await expect(page.locator(".wp-pane--schedule li")).toHaveCount(2);
  await page.locator(".lib-tabs").getByText("Scriptures").click();
  await page.locator(".lt-tree input").fill("God");
  const hits = page.locator(".lt-body .lt-row--verses");
  await expect(hits).toHaveCount(2);
  await hits.nth(0).click();
  await hits.nth(1).click({ modifiers: ["Control"] });
  await hits.nth(1).click({ button: "right" });
  await page.locator("[data-menu-depth='0'] button", { hasText: "Add 2 readings" }).click();
  await expect(page.locator(".wp-pane--schedule li")).toHaveCount(4);
});

test("free pictures online: key, search, download with the credit", async ({ page }) => {
  await page.goto("/#/control");
  await page.locator(".lib-tabs").getByText("Media").click();
  await page.locator(".media__where button", { hasText: "Online" }).click();
  await page.locator(".onl--key input[aria-label='Key']").fill("test-key");
  await page.locator(".onl--key button", { hasText: "Save key" }).click();
  await page.locator("input[aria-label='Search online']").fill("sunrise");
  await page.keyboard.press("Enter");
  await expect(page.locator(".onl__item")).toHaveCount(3);
  await page.locator(".onl__item button", { hasText: "Download" }).first().click();
  await page.locator(".media__where button", { hasText: "Library" }).click();
  await expect(page.locator(".wp-row--lib", { hasText: "by Kwame on Pixabay" })).toHaveCount(1);
});

test("headings group the service order and are stepped over", async ({ page }) => {
  page.on("dialog", (d) => void d.accept("Sermon"));
  await page.goto("/#/control");
  const rows = page.locator(".lt-row--songs");
  await rows.nth(0).click();
  await rows.nth(1).click({ modifiers: ["Control"] });
  await rows.nth(1).click({ button: "right" });
  await page.locator("[data-menu-depth='0'] button", { hasText: "Add 2 songs" }).click();
  const sched = page.locator(".wp-pane--schedule li");
  await sched.nth(0).click();
  await page.locator("button[aria-label='Add a heading']").click();
  await expect(page.locator(".wp-row--header")).toContainText("Sermon");
  await expect(sched.nth(2)).toContainText("02");
  await sched.nth(0).click();
  await page.locator(".wp-live-surface").focus();
  await page.keyboard.press("PageDown");
  await expect(page.locator(".ew-pane[aria-label=Preview] .wp-pane__sub")).toContainText("MHB 50");
});

test("find and replace previews the change", async ({ page }) => {
  await page.goto("/#/control");
  // from the Songs menu (waits for the list, unlike a key pressed at once)
  await page.locator(".lt-row--songs").first().click({ button: "right" });
  await page.locator("[data-menu-depth='0'] button", { hasText: "Find and replace in songs" }).click();
  await page.locator("input[aria-label='Find']").fill("cheerful");
  await page.locator("input[aria-label='Replace with']").fill("joyful");
  await page.locator("button", { hasText: "Preview the changes" }).click();
  await expect(page.locator(".frp__list")).toContainText("joyful songs");
});

test("an EasyWorship schedule opens as a new service", async ({ page }) => {
  await page.goto("/#/control");
  await expect(page.locator(".lt-row--songs").first()).toBeVisible();
  await page.keyboard.press("Control+k");
  await page.keyboard.type("EasyWorship schedule");
  await page.keyboard.press("Enter");
  await expect(page.locator(".wp-pane--schedule li")).toHaveCount(3);
  await expect(page.locator(".wp-row--header")).toContainText("from EasyWorship");
});

test("a custom stage layout is made from boxes", async ({ page }) => {
  await page.goto("/#/control");
  await page.locator("button", { hasText: "Settings" }).first().click();
  await page.locator("button", { hasText: "Stage display" }).click();
  await page.locator(".sle__mode label", { hasText: "Custom" }).locator("input").check();
  await page.locator(".sle__mode select").selectOption("band");
  await expect(page.locator(".sle__box")).toHaveCount(5);
  await expect(page.locator(".sle__screen .stg-box--clock")).toBeVisible();
  await page.locator(".sle__side select").first().selectOption("timer");
  await expect(page.locator(".sle__box")).toHaveCount(6);
  await page.locator(".sle .wp-btn--primary").click();
  await expect(page.locator(".sle .wp-btn--primary")).toHaveText("Saved");
});

test("the outputs dashboard and Check system point to the fix", async ({ page }) => {
  await page.goto("/#/control");
  await expect(page.locator(".odots .odot")).toHaveCount(3);
  await page.locator(".odots").click();
  await expect(page.locator(".opn__tile--silent")).toContainText("Not answering");
  await page.locator(".opn button", { hasText: "Check system" }).click();
  await expect(page.locator(".opn__sum")).toContainText("1 problem");
  await page.locator(".opn__item", { hasText: "Backup" }).locator("button", { hasText: "Fix" }).click();
  await expect(page.locator("h2", { hasText: "Backups" })).toBeVisible();
});
