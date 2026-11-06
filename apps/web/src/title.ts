export type Link = { start: number; end: number; url: string };

export type Part = { text: string; from: number; to: number; className?: string; href?: string };

const OPENER: Record<string, string> = { ")": "(", "]": "[" };

function urlEnd() {
  const depth: Record<string, number> = { "(": 0, "[": 0 };
  return (ch: string) => {
    if (ch < "!" || ch > "~" || ch === '"' || ch === "`" || ch === "<" || ch === ">") return true;
    if (ch === "(" || ch === "[") depth[ch] += 1;
    const opener = OPENER[ch];
    if (!opener) return false;
    if (depth[opener] === 0) return true;
    depth[opener] -= 1;
    return false;
  };
}

function hostOf(url: string) {
  const userinfoAndHost = url.slice(url.indexOf("://") + 3).split(/[/\\?#]/, 1)[0];
  return userinfoAndHost.slice(userinfoAndHost.lastIndexOf("@") + 1);
}

export function linkLabel(url: string) {
  return hostOf(url).toLowerCase().replace(/^www\.(?=.)/, "");
}

export function findLinks(title: string): Link[] {
  const links: Link[] = [];
  for (const match of title.matchAll(/https?:\/\//gi)) {
    const start = match.index;
    if (start < (links.at(-1)?.end ?? 0)) continue;
    const ends = urlEnd();
    let end = start + match[0].length;
    while (end < title.length && !ends(title[end])) end += 1;
    const url = title.slice(start, end);
    if (hostOf(url)) links.push({ start, end, url });
  }
  return links;
}

const utf8 = new TextEncoder();

function percentEncode(ch: string) {
  return Array.from(utf8.encode(ch), (byte) => `%${byte.toString(16).toUpperCase().padStart(2, "0")}`).join("");
}

function asciiHost(server: string) {
  if (!/[^\x00-\x7f]/.test(server)) return server;
  const colon = server.lastIndexOf(":");
  const host = colon < 0 ? server : server.slice(0, colon);
  const port = colon < 0 ? "" : server.slice(colon);
  try {
    return new URL(`http://${host}`).hostname + port;
  } catch {
    return null;
  }
}

export function encodePastedUrl(text: string): string | null {
  const pasted = text.trim();
  const scheme = /^https?:\/\//i.exec(pasted)?.[0];
  if (!scheme || /\s/.test(pasted)) return null;
  const rest = pasted.slice(scheme.length);
  const split = rest.search(/[/?#]/);
  const head = split < 0 ? rest : rest.slice(0, split);
  const at = head.lastIndexOf("@") + 1;
  const server = asciiHost(head.slice(at));
  if (!server) return null;
  const ends = urlEnd();
  let url = scheme;
  for (const ch of head.slice(0, at) + server + rest.slice(head.length)) url += ends(ch) ? percentEncode(ch) : ch;
  return url;
}

export function tokens(title: string, query: string): Part[] {
  const q = query.trim().toLowerCase();
  const parts: Part[] = [];
  const push = (text: string, from: number, className?: string) => parts.push({ text, from, to: from + text.length, className });
  const plain = (text: string, from: number) => {
    let rest = text;
    while (rest) {
      const at = q ? rest.toLowerCase().indexOf(q) : -1;
      if (at < 0) {
        push(rest, from);
        break;
      }
      if (at > 0) push(rest.slice(0, at), from);
      push(rest.slice(at, at + q.length), from + at, "match");
      from += at + q.length;
      rest = rest.slice(at + q.length);
    }
  };
  let at = 0;
  for (const link of findLinks(title)) {
    plain(title.slice(at, link.start), at);
    parts.push({
      text: linkLabel(link.url),
      from: link.start,
      to: link.end,
      className: q && link.url.toLowerCase().includes(q) ? "link match" : "link",
      href: link.url,
    });
    at = link.end;
  }
  plain(title.slice(at), at);
  return parts;
}
