import { describe, expect, it } from "vite-plus/test";
import { encodePastedUrl, findLinks, linkLabel, tokens } from "./title";

const urls = (title: string) => findLinks(title).map((link) => link.url);

describe("findLinks", () => {
  it("ends a link at full-width punctuation and typographic quotes so text written right after a URL is not opened as part of it", () => {
    expect(urls("PR: https://example.com/project/pull/296（two commits）")).toEqual(["https://example.com/project/pull/296"]);
    expect(urls("«preview: http://localhost:6420»: done")).toEqual(["http://localhost:6420"]);
    expect(urls("„https://example.com/pull/26“ (`Closes #25`)")).toEqual(["https://example.com/pull/26"]);
  });

  it("ends a link at the first Chinese, Japanese or Korean character so no space is needed after a URL", () => {
    expect(findLinks("詳細はhttps://example.com/を参照")).toEqual([{ start: 3, end: 23, url: "https://example.com/" }]);
    expect(urls("详情见https://example.com/。")).toEqual(["https://example.com/"]);
    expect(urls("자세한 내용은 https://example.com/에서 확인")).toEqual(["https://example.com/"]);
  });

  it("keeps trailing punctuation because real URLs end with it", () => {
    expect(urls("https://en.wikipedia.org/wiki/Washington,_D.C.")).toEqual(["https://en.wikipedia.org/wiki/Washington,_D.C."]);
    expect(urls("https://example.com/%E3%81%82%E3%82%8A%E3%81%8C%E3%81%A8%E3%81%86%E3%80%82")).toEqual([
      "https://example.com/%E3%81%82%E3%82%8A%E3%81%8C%E3%81%A8%E3%81%86%E3%80%82",
    ]);
  });

  it("keeps brackets that pair up inside the URL but not a closing bracket that wraps it", () => {
    expect(urls("(https://en.wikipedia.org/wiki/Mercury_(planet))")).toEqual(["https://en.wikipedia.org/wiki/Mercury_(planet)"]);
    expect(urls("[https://example.com/a[1]]")).toEqual(["https://example.com/a[1]"]);
  });

  it("ends a link at quotes, backticks and angle brackets, which cannot appear in a URL", () => {
    expect(urls('"https://example.com" and <https://example.org>')).toEqual(["https://example.com", "https://example.org"]);
    expect(urls("Spec: `https://developer.mozilla.org/en-US/docs/Web/API/URL/`")).toEqual(["https://developer.mozilla.org/en-US/docs/Web/API/URL/"]);
  });

  it("treats a URL inside another URL's query as part of it", () => {
    expect(urls("https://a.example/login?next=https://b.example/")).toEqual(["https://a.example/login?next=https://b.example/"]);
  });

  it("ignores a scheme with no host, which cannot be opened", () => {
    expect(urls("https:// and https:///path")).toEqual([]);
  });

  it("finds a scheme in any letter case", () => {
    expect(urls("HTTPS://Example.com")).toEqual(["HTTPS://Example.com"]);
  });
});

describe("linkLabel", () => {
  it("shows only the host so a long URL does not push the rest of the title out of the row", () => {
    expect(linkLabel("https://www.example.com:8080/very/long/path?q=1#top")).toBe("example.com:8080");
    expect(linkLabel("https://www.")).toBe("www.");
  });

  it("shows the host the browser will open, so text before an @ or a backslash cannot disguise where a link goes", () => {
    expect(linkLabel("https://bank.example@evil.example/login")).toBe("evil.example");
    expect(linkLabel("https://user:pass@www.example.com:8080/")).toBe("example.com:8080");
    expect(linkLabel("https://evil.example\\@bank.example/")).toBe("evil.example");
  });

  it("shows the host in lower case as the browser does, so a capital I cannot pass for a lower-case l", () => {
    expect(linkLabel("https://paypaI.example/")).toBe("paypai.example");
  });
});

describe("encodePastedUrl", () => {
  it("percent-encodes a pasted URL with letters outside ASCII so it stays one link even when it ends with one, such as 。", () => {
    const pasted = {
      "https://example.com/ありがとう。": "https://example.com/%E3%81%82%E3%82%8A%E3%81%8C%E3%81%A8%E3%81%86%E3%80%82",
      "https://zh.wikipedia.org/wiki/北京": "https://zh.wikipedia.org/wiki/%E5%8C%97%E4%BA%AC",
      "https://ko.wikipedia.org/wiki/서울": "https://ko.wikipedia.org/wiki/%EC%84%9C%EC%9A%B8",
      "https://de.wikipedia.org/wiki/Fuß": "https://de.wikipedia.org/wiki/Fu%C3%9F",
    };
    for (const [raw, encoded] of Object.entries(pasted)) {
      expect(encodePastedUrl(raw)).toBe(encoded);
      expect(urls(encoded)).toEqual([encoded]);
    }
  });

  it("leaves an already encoded URL as it is so pasting it twice does not double-encode it", () => {
    expect(encodePastedUrl("https://ko.wikipedia.org/wiki/%EC%84%9C%EC%9A%B8")).toBe("https://ko.wikipedia.org/wiki/%EC%84%9C%EC%9A%B8");
  });

  it("converts a domain outside ASCII to punycode and keeps its port", () => {
    expect(encodePastedUrl("https://日本語.jp:8080/パス")).toBe("https://xn--wgv71a119e.jp:8080/%E3%83%91%E3%82%B9");
    expect(encodePastedUrl("https://münchen.de/straße")).toBe("https://xn--mnchen-3ya.de/stra%C3%9Fe");
    expect(encodePastedUrl("https://한국.kr/")).toBe("https://xn--3e0b707e.kr/");
  });

  it("drops the whitespace a copied URL often carries", () => {
    expect(encodePastedUrl("  https://example.com/北京\n")).toBe("https://example.com/%E5%8C%97%E4%BA%AC");
  });

  it("encodes characters that would otherwise end the link early so the whole pasted URL opens", () => {
    for (const pasted of ["https://example.com/a)b", "https://example.com/a))(b)", 'https://example.com/"x"', "https://example.com/😀"]) {
      const url = encodePastedUrl(pasted)!;
      expect(urls(url)).toEqual([url]);
    }
    expect(encodePastedUrl("https://example.com/a))(b)")).toBe("https://example.com/a%29%29(b)");
    expect(encodePastedUrl("https://example.com/😀")).toBe("https://example.com/%F0%9F%98%80");
  });

  it("leaves anything but a single URL alone because its extent cannot be known", () => {
    for (const pasted of ["München https://example.com/", "https://example.com/a b", "ftp://example.com/", "https://", "plain text"]) {
      expect(encodePastedUrl(pasted)).toBeNull();
    }
  });
});

describe("tokens", () => {
  it("shows a URL as its host but opens the whole URL, fragment included", () => {
    const title = "read https://example.com/page#section";
    expect(tokens(title, "").map(({ text, className, href }) => ({ text, className, href }))).toEqual([
      { text: "read ", className: undefined, href: undefined },
      { text: "example.com", className: "link", href: "https://example.com/page#section" },
    ]);
  });

  it("highlights the link when the filter matches only its hidden URL, so the reason the row is shown is visible", () => {
    expect(tokens("see https://ja.wikipedia.org/wiki/x", "wiki").map((part) => part.className)).toEqual([undefined, "link match"]);
  });

  it("covers the title in order without gaps so a click on any part maps back to a caret position", () => {
    const title = "Buy milk at https://shop.example/milk then pay";
    const parts = tokens(title, "a");
    expect(parts[0].from).toBe(0);
    expect(parts.at(-1)!.to).toBe(title.length);
    for (let i = 1; i < parts.length; i += 1) expect(parts[i].from).toBe(parts[i - 1].to);
    for (const part of parts.filter((p) => !p.href)) expect(title.slice(part.from, part.to)).toBe(part.text);
  });
});
