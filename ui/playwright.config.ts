import { defineConfig } from "@playwright/test";

// End-to-end checks of the operator's essentials against the built interface.
export default defineConfig({
  testDir: "e2e",
  timeout: 30_000,
  use: {
    baseURL: "http://127.0.0.1:4173",
    viewport: { width: 1500, height: 920 },
    // a browser already on this machine (PW_CHROMIUM), else Playwright's own
    launchOptions: process.env.PW_CHROMIUM ? { executablePath: process.env.PW_CHROMIUM } : {},
  },
  webServer: { command: "npx vite preview --port 4173 --strictPort", url: "http://127.0.0.1:4173", reuseExistingServer: true, timeout: 60_000 },
});
