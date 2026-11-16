import { build, defineConfig, type Plugin } from "vite-plus";
import { voidPlugin } from "void";
import { voidReact } from "@void/react/plugin";

const STATS_APP = "virtual:stats-app";

function statsApp(): Plugin {
  return {
    name: "nestodo:stats-app",
    resolveId: (id) => (id === STATS_APP ? `\0${STATS_APP}` : null),
    async load(id) {
      if (id !== `\0${STATS_APP}`) return null;
      const result = await build({
        configFile: false,
        logLevel: "warn",
        build: { write: false, cssCodeSplit: false, rolldownOptions: { input: "src/stats-app.tsx", output: { codeSplitting: false } } },
      });
      if (!("output" in result)) throw new Error("The stats app did not build into one bundle");
      const [script, ...assets] = result.output;
      const [style, ...others] = assets;
      if (style?.type !== "asset" || !style.fileName.endsWith(".css") || others.length > 0) {
        throw new Error(`The stats app must build into one script and one stylesheet, but built ${result.output.map((file) => file.fileName).join(", ")}`);
      }
      for (const file of script.moduleIds) if (!file.startsWith("\0") && !file.includes("/node_modules/")) this.addWatchFile(file);
      const html = [
        '<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Stats</title>',
        `<style>${style.source}</style></head><body><script type="module">${script.code.replaceAll("</script", "<\\/script")}</script></body></html>`,
      ].join("");
      return `export default ${JSON.stringify(html)};`;
    },
  };
}

export default defineConfig(({ mode }) => ({
  plugins: mode === "test" ? [] : [
    voidPlugin({ persistTo: process.env.E2E_PERSIST_TO }),
    voidReact({ react: { compiler: { panicThreshold: "all_errors" } } }),
    statsApp(),
  ],
  test: {
    projects: [
      { test: { name: "unit", include: ["src/**/*.test.ts"] } },
      { test: { name: "e2e", include: ["e2e/**/*.test.ts"], globalSetup: "e2e/global-setup.ts", fileParallelism: false } },
    ],
  },
}));
