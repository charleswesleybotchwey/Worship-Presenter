import { expect, test } from "@playwright/test";

// The operator's essentials, in the browser preview (the app's own stand-in data).

test("a song goes live and the switches bring the words back", async ({ page, context }) => {
  const out = await context.newPage();
  await out.goto("/#/output/main");
  await page.goto("/#/control");
  await page.locator(".lt-row--songs").first().click();
  await page.locator(".wp-btn--primary", { hasText: "Go live" }).click();
  await expect(out.locator(".tl-box").first()).toBeVisible();
  // the Clear switch, not "Clear all" (its label gains an ON tag while on)
  const clear = page.locator(".wp-service-actions button", { hasText: "Clear" }).filter({ hasNotText: "all" });
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
  await page.locator("button", { hasText: "Tools" }).click();
  await page.locator(".tools-menu__pop button[title^='The live transcript']").click();
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
  await page.locator(".lib-pane:not([hidden]) .lt-tree input").fill("God");
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

test("props show on their screens and Clear all takes everything off", async ({ page, context }) => {
  const out = await context.newPage();
  await out.goto("/#/output/main");
  await page.goto("/#/control");
  await page.locator(".lt-row--songs").first().click();
  await page.locator(".wp-btn--primary", { hasText: "Go live" }).click();
  await expect(out.locator(".tl-box").first()).toBeVisible();
  await page.locator(".wp-service-actions button", { hasText: "Props" }).click();
  await page.locator(".prp__row button.wp-mode", { hasText: "Sermon title" }).click();
  await page.locator(".prp__row button.wp-mode", { hasText: "LIVE badge" }).click();
  await expect(out.locator(".prop")).toHaveText(["Grace upon grace — John 1:16"]); // the LIVE badge is for the stream only
  await page.locator(".opn button", { hasText: "Close" }).click();
  await page.locator(".wp-service-actions button", { hasText: "Clear all" }).click();
  await expect(out.locator(".prop")).toHaveCount(0);
  await expect(out.locator(".tl-box")).toHaveCount(0);
});

test("group buttons jump to a section; the arrangement is built from blocks", async ({ page }) => {
  await page.goto("/#/control");
  await page.locator(".lt-row--songs", { hasText: "Blessed Assurance" }).click();
  await page.locator(".wp-btn--primary", { hasText: "Go live" }).click();
  await expect(page.locator(".grp-buttons .grp-btn")).toHaveText(["V1", "C", "V2", "B"]);
  await page.locator(".grp-buttons .grp-btn", { hasText: "B" }).click();
  await expect(page.locator(".ew-pane[aria-label=Live] .sl-group.is-current .sl-head")).toHaveText("Bridge");
  await page.locator(".lt-row--songs", { hasText: "Blessed Assurance" }).click({ button: "right" });
  await page.locator("[data-menu-depth='0'] button", { hasText: "Edit" }).first().click();
  await page.locator(".arr button", { hasText: "Chorus after each verse" }).click();
  await expect(page.locator("input[placeholder^='As written']")).toHaveValue("V1 C V2 C B C");
});

test("a macro runs its steps; a slide action runs a macro when its slide goes live", async ({ page, context }) => {
  const out = await context.newPage();
  await out.goto("/#/output/main");
  await page.goto("/#/control");
  await page.locator(".lt-row--songs", { hasText: "Blessed Assurance" }).click();
  await page.locator(".ew-pane[aria-label=Preview] .sl-group", { hasText: "Bridge" }).locator(".sl-slide").first().click({ button: "right" });
  await page.locator("[role=menuitem]", { hasText: "Walk-in" }).click();
  await expect(page.locator(".ew-pane[aria-label=Preview] .sl-group", { hasText: "Bridge" }).locator(".sl-action")).toHaveCount(1);
  await page.locator(".wp-btn--primary", { hasText: "Go live" }).click();
  await page.locator(".wp-service-actions button", { hasText: "Macros" }).click();
  await page.locator(".mac__run", { hasText: "Sermon" }).click();
  await expect(out.locator(".prop")).toHaveText(["Grace upon grace — John 1:16"]);
  await page.locator(".opn button", { hasText: "Close" }).click();
  await page.locator(".grp-buttons .grp-btn", { hasText: "B" }).click();
  await expect(page.locator(".wp-service-actions button", { hasText: "Logo" })).toHaveAttribute("aria-pressed", "true");
  await expect(out.locator(".prop")).toHaveCount(2);
});

test("a timeline records slide changes against a track and plays them back", async ({ page }) => {
  await page.goto("/#/control");
  await page.locator(".lt-row--songs", { hasText: "Blessed Assurance" }).click();
  await page.locator(".wp-btn--primary", { hasText: "Go live" }).click();
  await page.locator(".tlb__track").selectOption({ label: "🎞 Motion loop blue" });
  await expect(page.locator(".tlb__time")).toContainText("0:04");
  await page.locator(".tlb button", { hasText: "Record" }).click();
  const slides = page.locator(".ew-pane[aria-label=Live] .sl-slide");
  for (const i of [1, 2]) {
    await page.waitForTimeout(900);
    await slides.nth(i).click();
  }
  await page.locator(".tlb button", { hasText: "Stop" }).click();
  await expect(page.locator(".tlb__bar")).toHaveAttribute("title", "3 slide changes recorded");
  await slides.nth(0).click();
  await page.locator(".tlb button", { hasText: "Play" }).click();
  await expect(page.locator(".ew-pane[aria-label=Live] .sl-slide.is-current .sl-num")).toContainText("3", { timeout: 6000 });
});

test("a live video crossfades into the next item, still playing as it fades", async ({ page, context }) => {
  const out = await context.newPage();
  await out.goto("/#/output/main");
  await page.goto("/#/control");
  await page.locator(".lib-tabs").getByText("Media").click();
  await page.locator(".wp-row--lib", { hasText: "Motion" }).click();
  await page.locator(".wp-btn--primary", { hasText: "Go live" }).click();
  await expect(out.locator("video")).toHaveCount(1);
  await page.locator(".lib-tabs").getByText("Songs").click();
  await page.locator(".lt-row--songs", { hasText: "Blessed Assurance" }).click();
  await page.locator(".wp-btn--primary", { hasText: "Go live" }).click();
  await expect(out.locator(".xf__out video")).toHaveCount(1); // the outgoing video, fading
  await expect(out.locator(".xf__out")).toHaveCount(0, { timeout: 3000 }); // then gone
  await expect(out.locator(".tl-text").first()).toBeVisible();
});

test("the theme editor's property rows stay in place (no stray styles)", async ({ page }) => {
  await page.goto("/#/control");
  await page.locator(".lib-tabs").getByText("Themes").click();
  await page.locator(".lt-theme").first().dblclick();
  const rows = page.locator(".te .prop, .prop-group .prop");
  await expect(rows.first()).toBeVisible();
  const positions = await rows.evaluateAll((els) => els.slice(0, 8).map((e) => getComputedStyle(e).position));
  expect(positions.every((p) => p !== "absolute")).toBe(true);
});

test("the library keeps its place, marks what is live, and the window never scrolls", async ({ page }) => {
  await page.goto("/#/control");
  const vis = ".lib-pane:not([hidden]) ";
  await page.locator(".lt-row--songs", { hasText: "Blessed Assurance" }).click();
  await page.locator(".wp-btn--primary", { hasText: "Go live" }).click();
  await page.locator(".lt-row--songs", { hasText: "MHB 50" }).click();
  await expect(page.locator(".lt-row--songs.is-on-air")).toContainText("Blessed Assurance");
  await expect(page.locator(".lt-row--songs.is-in-preview")).toContainText("MHB 50");
  await page.locator(".lib-tabs").getByText("Scriptures").click();
  await page.locator(vis + ".lt-node", { hasText: "TK" }).click();
  await page.locator(".lib-tabs").getByText("Songs").click();
  await expect(page.locator(vis + ".lt-row--songs.is-selected")).toContainText("MHB 50"); // still where it was
  await page.locator(".lib-tabs").getByText("Scriptures").click();
  await expect(page.locator(vis + ".lt-node.is-selected").first()).toContainText("TK"); // the same Bible
  await page.mouse.wheel(0, 3000);
  expect(await page.evaluate(() => document.scrollingElement!.scrollTop)).toBe(0);
});

test("the song editor opens and takes a language typed in", async ({ page }) => {
  await page.goto("/#/control");
  await page.locator(".lt-row--songs", { hasText: "MHB 50" }).click({ button: "right" });
  await page.locator("[data-menu-depth='0'] button", { hasText: "Edit" }).first().click();
  page.once("dialog", (d) => void d.accept("Kasem"));
  await page.locator("select[aria-label='Add a second language']").selectOption("__other");
  await expect(page.locator("select[aria-label=\"This tab's language\"] option:checked")).toHaveText("Language: Kasem");
});

test("a Worship Presenter package is exported, reviewed and imported", async ({ page }) => {
  await page.goto("/#/control");
  // the window is ready (its keys work) once the library has drawn
  await expect(page.locator(".lt-row--songs").first()).toBeVisible();
  const command = async (text: string) => {
    await page.keyboard.press("Control+k");
    const box = page.locator(".palette__input");
    await expect(box).toBeVisible();
    await box.fill(text);
    await page.keyboard.press("Enter");
  };
  await command("Export a Worship Presenter package");
  await page.locator(".pkg button", { hasText: "Export…" }).click();
  await expect(page.locator(".pkg")).toContainText("Saved");
  await page.locator(".pkg button", { hasText: "Close" }).click();
  await command("Import a Worship Presenter package");
  await page.locator(".pkg button", { hasText: "Choose a package" }).click();
  await expect(page.locator(".pkg__list")).toContainText("in more than one language");
  await page.locator(".pkg button", { hasText: "Import" }).click();
  await expect(page.locator(".pkg__done")).toContainText("songs added");
});

test("songs export one at a time or several, as OpenLyrics one file each", async ({ page }) => {
  await page.goto("/#/control");
  await page.locator(".lt-row--songs", { hasText: "MHB 50" }).click();
  await page.locator(".lt-row--songs", { hasText: "Blessed Assurance" }).click({ modifiers: ["Control"] });
  await page.locator(".lt-row--songs", { hasText: "Blessed Assurance" }).click({ button: "right" });
  await page.locator("[data-menu-depth='0'] button", { hasText: "Export 2 songs" }).click();
  await page.locator("input[name=song-format]").nth(1).check();
  await expect(page.locator(".pkg input[type=checkbox]")).toBeChecked(); // OpenLyrics: a file each
  await page.locator(".pkg button", { hasText: "Export…" }).click();
  await expect(page.locator(".pkg__done")).toHaveText("2 files saved in the folder.");
});

test("spelling in Fante is checked against the library, and fixed", async ({ page }) => {
  await page.goto("/#/control");
  await page.locator(".lt-row--songs", { hasText: "MHB 1," }).click({ button: "right" });
  await page.locator("[data-menu-depth='0'] button", { hasText: "Edit" }).first().click();
  await page.locator("button", { hasText: "Fante" }).first().click();
  const ta = page.locator("textarea").first();
  await ta.fill((await ta.inputValue()) + "\nO Nyankopon, yɛ da wo ase");
  await page.locator("button", { hasText: "Check spelling" }).click();
  await expect(page.locator(".spell__row")).toContainText("nyankopɔn");
  await page.locator(".spell__row button", { hasText: "Fix" }).click();
  await expect(ta).toHaveValue(/O Nyankopɔn, yɛ da wo ase/);
});

test("a hymn board screen lists the service's hymn numbers", async ({ page }) => {
  await page.goto("/#/output/screen-hymns");
  await expect(page.locator(".hymn-board__num").first()).toHaveText("MHB 1");
});

test("the lectionary shows the day and adds its readings to the service", async ({ page }) => {
  await page.goto("/#/control");
  await expect(page.locator(".lt-row--songs").first()).toBeVisible();
  await page.keyboard.press("Control+k");
  await expect(page.locator(".palette__input")).toBeVisible();
  await page.locator(".palette__input").fill("Lectionary readings");
  await page.keyboard.press("Enter");
  await expect(page.locator(".lect__day")).toContainText("Year B");
  await expect(page.locator(".lect__list li").last()).toBeInViewport(); // not pushed out of sight
  await page.locator(".lect button", { hasText: "Add all" }).click();
  await page.locator(".lect button", { hasText: "Close" }).click();
  await expect(page.locator(".wp-pane--schedule .wp-row")).toHaveCount(5); // the psalm's two ranges as two items
});

test("a hymn's tune plays once per verse, the slides moving with it", async ({ page }) => {
  await page.goto("/#/control");
  await page.locator(".lt-row--songs", { hasText: "Blessed Assurance" }).click();
  await page.locator(".wp-btn--primary", { hasText: "Go live" }).click();
  await page.locator("select[aria-label=\"The song's tune\"]").selectOption({ label: "🎞 Motion loop blue" });
  await page.locator(".tune button", { hasText: "Play verse by verse" }).click();
  await expect(page.locator(".tune .tune__time")).toHaveText("verse 1 of 2");
  await expect(page.locator(".tune .tune__time")).toHaveText("verse 2 of 2", { timeout: 12_000 });
  await expect(page.locator(".ew-pane[aria-label=Live] .sl-slide.is-current .sl-num")).toContainText("4", { timeout: 8000 });
});

test("a large-print screen shows bigger, bold words", async ({ page, context }) => {
  const normal = await context.newPage();
  const large = await context.newPage();
  await normal.goto("/#/output/main");
  await large.goto("/#/output/screen-large");
  await page.goto("/#/control");
  await page.locator(".lt-row--songs", { hasText: "MHB 1," }).click();
  await page.locator(".wp-btn--primary", { hasText: "Go live" }).click();
  const size = (p: typeof page) =>
    p.locator('.tl-text[data-field="lyrics"]').first().evaluate((el) => {
      const leaf = [...el.querySelectorAll("*")].find((e) => e.childElementCount === 0 && e.textContent?.trim()) ?? el;
      return [parseFloat(getComputedStyle(leaf).fontSize), getComputedStyle(leaf).fontWeight];
    });
  await expect(large.locator(".output--large")).toBeVisible();
  const [n, l] = [await size(normal), await size(large)];
  expect(l[0]).toBeGreaterThan(n[0] * 1.2);
  expect(l[1]).toBe("700");
});
