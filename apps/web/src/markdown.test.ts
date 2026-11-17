import { describe, expect, it } from "vite-plus/test";
import { markdown, type Block, type Inline } from "./markdown";

const text = (value: string): Inline => ({ type: "text", text: value });
const code = (value: string): Inline => ({ type: "code", text: value });
const link = (href: string, ...children: Inline[]): Inline => ({ type: "link", href, children });
const styled = (type: "strong" | "em" | "del", ...children: Inline[]): Inline => ({ type, children });
const p = (...children: Inline[]): Block => ({ type: "paragraph", children });
const list = (start: number | null, ...items: Block[][]): Block => ({ type: "list", start, items });

describe("markdown", () => {
  it("keeps every line break and run of spaces inside a paragraph, so a log pasted without Markdown in mind reads as it was written", () => {
    expect(markdown("built  ok\ntests   2 failed")).toEqual([p(text("built  ok\ntests   2 failed"))]);
  });

  it("formats bold, italic, struck and code text, the marks an assistant writes a summary with", () => {
    expect(markdown("**done** and *maybe*, ~~dropped~~, run `vp test`")).toEqual([
      p(styled("strong", text("done")), text(" and "), styled("em", text("maybe")), text(", "), styled("del", text("dropped")), text(", run "), code("vp test")),
    ]);
  });

  it("formats marks that touch Japanese text, which has no spaces to set them apart", () => {
    expect(markdown("これは**太字**です")).toEqual([p(text("これは"), styled("strong", text("太字")), text("です"))]);
  });

  it("leaves a single tilde alone, so a range such as 10~20 is not struck through", () => {
    expect(markdown("10~20 items, 30~40 at most")).toEqual([p(text("10~20 items, 30~40 at most"))]);
  });

  it("nests lists and starts a numbered list at the number written, so steps keep their order when a comment continues an earlier one", () => {
    expect(markdown("3. build\n   - web\n   - cli\n4. release")).toEqual([
      list(3, [p(text("build")), list(null, [p(text("web"))], [p(text("cli"))])], [p(text("release"))]),
    ]);
  });

  it("shows a fenced block exactly as written, so pasted output keeps its indentation and its asterisks", () => {
    expect(markdown("```sh\n$ ls  *.ts\n  a.ts  **b.ts**\n```")).toEqual([{ type: "code", text: "$ ls  *.ts\n  a.ts  **b.ts**" }]);
  });

  it("reads headings, quotes and rules, so a long comment can be divided into parts", () => {
    expect(markdown("## Plan\n\n> wait for the review\n\n---")).toEqual([
      { type: "heading", level: 2, children: [text("Plan")] },
      { type: "quote", children: [p(text("wait for the review"))] },
      { type: "rule" },
    ]);
  });

  it("shows a link as its label, with the label's own formatting, and opens the address behind it", () => {
    expect(markdown("see [the **pull** request](https://example.com/pull/7) first")).toEqual([
      p(text("see "), link("https://example.com/pull/7", text("the "), styled("strong", text("pull")), text(" request")), text(" first")),
    ]);
  });

  it("turns a URL written on its own into a link that shows it as written, so a comment that is only a link can still be opened", () => {
    const url = "https://example.com/a_b_c";
    expect(markdown(url)).toEqual([p(link(url, text(url)))]);
    expect(markdown("<https://example.org> and **https://example.net/x**")).toEqual([
      p(link("https://example.org", text("https://example.org")), text(" and "), styled("strong", link("https://example.net/x", text("https://example.net/x")))),
    ]);
  });

  it("ends a URL written on its own where Japanese text starts, so a sentence that goes on after it is not opened as part of the address", () => {
    expect(markdown("詳細はhttps://example.com/を参照")).toEqual([p(text("詳細は"), link("https://example.com/", text("https://example.com/")), text("を参照"))]);
  });

  it("leaves a URL inside code or inside a link's label as text, so code reads as written and one link is never nested in another", () => {
    expect(markdown("`curl https://example.com/api` or [see https://example.com/a](https://example.com/b)")).toEqual([
      p(code("curl https://example.com/api"), text(" or "), link("https://example.com/b", text("see https://example.com/a"))),
    ]);
  });

  it("keeps a picture inside a link as its description, so one link is never nested in another", () => {
    expect(markdown("[![logo](https://example.com/logo.png) home](https://example.com)")).toEqual([p(link("https://example.com", text("logo home")))]);
  });

  it("opens the address a link names when it is written with character references", () => {
    expect(markdown("AT&amp;T [search](https://example.com/?q=1&amp;page=2)")).toEqual([p(text("AT&T "), link("https://example.com/?q=1&page=2", text("search")))]);
  });

  it("leaves a link that is not http or https as written, so nothing in a comment runs a script or opens another app", () => {
    const written = "[run](javascript:alert(1)), [mail](mailto:a@example.com), [here](/settings) and <a@example.com>";
    expect(markdown(written)).toEqual([p(text(written))]);
  });

  it("shows only the label of a link with no address, so nothing that looks like a link opens nowhere", () => {
    expect(markdown("[later]() and ![chart]()")).toEqual([p(text("later and chart"))]);
  });

  it("shows HTML, tables and task boxes as written, since they are not drawn", () => {
    expect(markdown("<b>bold</b>\n\n<script>alert(1)</script>\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [ ] later")).toEqual([
      p(text("<b>bold</b>")),
      p(text("<script>alert(1)</script>")),
      p(text("| a | b |\n|---|---|\n| 1 | 2 |")),
      list(null, [p(text("[ ] later"))]),
    ]);
  });

  it("turns a picture into a link to it labelled with its description, since comments do not load pictures", () => {
    expect(markdown("![the chart](https://example.com/chart.png)")).toEqual([p(link("https://example.com/chart.png", text("the chart")))]);
  });

  it("follows a reference to the address defined for it elsewhere in the comment", () => {
    expect(markdown("see [the doc][1] today\n\n[1]: https://example.com/doc")).toEqual([
      p(text("see "), link("https://example.com/doc", text("the doc")), text(" today")),
    ]);
  });

  it("shows a comment that is only a link definition or an empty heading as written, so no comment is ever blank", () => {
    expect(markdown("[1]: https://example.com/doc")).toEqual([p(text("[1]: "), link("https://example.com/doc", text("https://example.com/doc")))]);
    expect(markdown("#")).toEqual([p(text("#"))]);
  });
});
