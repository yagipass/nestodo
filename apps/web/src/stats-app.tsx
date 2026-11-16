import { createRoot } from "react-dom/client";
import { App, type McpUiHostContext, type McpUiToolResultNotification } from "@modelcontextprotocol/ext-apps";
import type { Day } from "./stats";
import { StatsView } from "./stats-view";
import "./styles.css";

function daysIn(result: McpUiToolResultNotification["params"]) {
  const [block] = result.content;
  if (result.isError || block?.type !== "text") return null;
  return (JSON.parse(block.text) as { days: Day[] }).days;
}

function followTheme({ theme }: McpUiHostContext) {
  if (theme) document.documentElement.dataset.appearance = theme;
}

const root = createRoot(document.body.appendChild(document.createElement("div")));
const app = new App({ name: "nestodo-stats", version: "1.0.0" });

app.ontoolresult = (result) => {
  const days = daysIn(result);
  root.render(<main className="stats embedded">{days ? <StatsView days={days} /> : <p>Stats are not available.</p>}</main>);
};
app.onhostcontextchanged = followTheme;

await app.connect();
followTheme(app.getHostContext() ?? {});
