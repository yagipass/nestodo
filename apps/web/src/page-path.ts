const NOT_PAGES = ["/api/", "/__void/"];
const FILE = /\/[^/]*\.[^/]*$/;

export function isPagePath(path: string) {
  return path !== "/mcp" && !NOT_PAGES.some((prefix) => path.startsWith(prefix)) && !FILE.test(path);
}
