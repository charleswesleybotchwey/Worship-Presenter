import { defineConfig } from "vitest/config";

// unit tests only (the operator workflows are tested with Playwright in e2e/)
export default defineConfig({ test: { include: ["src/**/*.test.ts"] } });
