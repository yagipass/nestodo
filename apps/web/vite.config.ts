import { defineConfig } from "vite-plus";
import { voidPlugin } from "void";
import { voidReact } from "@void/react/plugin";

export default defineConfig(({ mode }) => ({
  plugins: mode === "test" ? [] : [
    voidPlugin({ persistTo: process.env.E2E_PERSIST_TO }),
    voidReact({ react: { compiler: { panicThreshold: "all_errors" } } }),
  ],
  test: {
    projects: [
      { test: { name: "unit", include: ["src/**/*.test.ts"] } },
      { test: { name: "e2e", include: ["e2e/**/*.test.ts"], globalSetup: "e2e/global-setup.ts", fileParallelism: false } },
    ],
  },
}));
