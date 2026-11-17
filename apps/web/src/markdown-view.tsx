import { markdown, type Block, type Inline } from "./markdown";
import { Tokens } from "./tokens";

const HEADINGS = ["h4", "h5", "h6"] as const;

const NO_LINKS = [] as const;

function Inlines({ nodes, query }: { nodes: readonly Inline[]; query: string }) {
  return nodes.map((node, index) => {
    switch (node.type) {
      case "text":
        return <Tokens key={index} text={node.text} query={query} links={NO_LINKS} />;
      case "code":
        return (
          <code key={index}>
            <Tokens text={node.text} query={query} links={NO_LINKS} />
          </code>
        );
      case "link":
        return (
          <a key={index} href={node.href} target="_blank" rel="noreferrer">
            <Inlines nodes={node.children} query={query} />
          </a>
        );
      default: {
        const Tag = node.type;
        return (
          <Tag key={index}>
            <Inlines nodes={node.children} query={query} />
          </Tag>
        );
      }
    }
  });
}

function Blocks({ nodes, query }: { nodes: readonly Block[]; query: string }) {
  return nodes.map((node, index) => {
    switch (node.type) {
      case "paragraph":
        return (
          <p key={index}>
            <Inlines nodes={node.children} query={query} />
          </p>
        );
      case "heading": {
        const Tag = HEADINGS[Math.min(node.level, HEADINGS.length) - 1];
        return (
          <Tag key={index}>
            <Inlines nodes={node.children} query={query} />
          </Tag>
        );
      }
      case "code":
        return (
          <pre key={index}>
            <code>
              <Tokens text={node.text} query={query} links={NO_LINKS} />
            </code>
          </pre>
        );
      case "quote":
        return (
          <blockquote key={index}>
            <Blocks nodes={node.children} query={query} />
          </blockquote>
        );
      case "list": {
        const items = node.items.map((item, at) => (
          <li key={at}>
            <Blocks nodes={item} query={query} />
          </li>
        ));
        return node.start === null ? (
          <ul key={index}>{items}</ul>
        ) : (
          <ol key={index} start={node.start}>
            {items}
          </ol>
        );
      }
      case "rule":
        return <hr key={index} />;
    }
  });
}

export function Markdown({ text, query }: { text: string; query: string }) {
  return (
    <div className="thread-text">
      <Blocks nodes={markdown(text)} query={query} />
    </div>
  );
}
