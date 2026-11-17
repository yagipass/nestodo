import MarkdownIt, { type Token } from "markdown-it";
import { findLinks } from "./title";

export type Inline =
  | { type: "text"; text: string }
  | { type: "code"; text: string }
  | { type: "strong" | "em" | "del"; children: Inline[] }
  | { type: "link"; href: string; children: Inline[] };

export type Block =
  | { type: "paragraph"; children: Inline[] }
  | { type: "heading"; level: number; children: Inline[] }
  | { type: "code"; text: string }
  | { type: "quote"; children: Block[] }
  | { type: "list"; start: number | null; items: Block[][] }
  | { type: "rule" };

const STYLES = { strong_open: "strong", em_open: "em", s_open: "del" } as const;

function opens(url: string) {
  return /^https?:\/\//i.test(url) && URL.canParse(url);
}

const parser = new MarkdownIt("commonmark", { html: false }).enable("strikethrough");

parser.validateLink = opens;

function linkTo(href: string, label: Inline[]): Inline {
  return { type: "link", href, children: label.length > 0 ? label : [{ type: "text", text: href }] };
}

function withBareLinks(nodes: readonly Inline[]): Inline[] {
  return nodes.flatMap((node): Inline[] => {
    if (node.type !== "text") return [node];
    const parts: Inline[] = [];
    let at = 0;
    for (const { start, end, url } of findLinks(node.text)) {
      if (start > at) parts.push({ type: "text", text: node.text.slice(at, start) });
      parts.push(linkTo(url, []));
      at = end;
    }
    if (at < node.text.length) parts.push({ type: "text", text: node.text.slice(at) });
    return parts;
  });
}

function asWritten(text: string): Block {
  return { type: "paragraph", children: withBareLinks([{ type: "text", text }]) };
}

function inlines(tokens: readonly Token[]): Inline[] {
  let at = 0;

  function until(close: string | null, linked: boolean): Inline[] {
    const out: Inline[] = [];
    const add = (node: Inline) => {
      const last = out.at(-1);
      if (node.type !== "text") out.push(node);
      else if (last?.type === "text") last.text += node.text;
      else if (node.text !== "") out.push(node);
    };
    const text = (value: string) => add({ type: "text", text: value });
    while (at < tokens.length) {
      const token = tokens[at];
      at += 1;
      if (token.type === close) break;
      switch (token.type) {
        case "softbreak":
        case "hardbreak":
          text("\n");
          break;
        case "code_inline":
          add({ type: "code", text: token.content });
          break;
        case "strong_open":
        case "em_open":
        case "s_open":
          add({ type: STYLES[token.type], children: until(token.type.replace("_open", "_close"), linked) });
          break;
        case "link_open": {
          const href = String(token.attrGet("href"));
          const label = until("link_close", true);
          if (opens(href)) add(linkTo(href, label));
          else label.forEach(add);
          break;
        }
        case "image": {
          const src = String(token.attrGet("src"));
          const alt = (token.children ?? []).map((child) => child.content).join("");
          if (linked || !opens(src)) text(alt);
          else add(linkTo(src, alt === "" ? [] : [{ type: "text", text: alt }]));
          break;
        }
        default:
          text(token.content);
      }
    }
    return linked ? out : withBareLinks(out);
  }

  return until(null, false);
}

export function markdown(source: string): Block[] {
  const tokens = parser.parse(source, {});
  let at = 0;

  function until(close: string | null): Block[] {
    const out: Block[] = [];
    while (at < tokens.length) {
      const token = tokens[at];
      at += 1;
      if (token.type === close) break;
      switch (token.type) {
        case "paragraph_open":
        case "heading_open": {
          const children = inlines(tokens[at].children ?? []);
          at += 2;
          if (token.type === "paragraph_open") out.push({ type: "paragraph", children });
          else if (children.length === 0) out.push(asWritten(token.markup));
          else out.push({ type: "heading", level: Number(token.tag.slice(1)), children });
          break;
        }
        case "fence":
        case "code_block":
          out.push({ type: "code", text: token.content.replace(/\n$/, "") });
          break;
        case "blockquote_open":
          out.push({ type: "quote", children: until("blockquote_close") });
          break;
        case "bullet_list_open":
        case "ordered_list_open": {
          const items: Block[][] = [];
          while (tokens[at].type === "list_item_open") {
            at += 1;
            items.push(until("list_item_close"));
          }
          at += 1;
          out.push({ type: "list", start: token.type === "ordered_list_open" ? Number(token.attrGet("start") ?? 1) : null, items });
          break;
        }
        case "hr":
          out.push({ type: "rule" });
          break;
        default:
          out.push(asWritten(token.content));
      }
    }
    return out;
  }

  const shown = until(null);
  return shown.length > 0 ? shown : [asWritten(source.trim())];
}
