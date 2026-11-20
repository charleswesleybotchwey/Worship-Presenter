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
  // the preview server listens on 127.0.0.1 exactly: "localhost" can be IPv6 (::1) on
  // newer machines, where the server would run but never answer here
  webServer: {
    command: "npx vite preview --host 127.0.0.1 --port 4173 --strictPort",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 520_000,
    stdout: "pipe",
    stderr: "pipe",
  },
});
