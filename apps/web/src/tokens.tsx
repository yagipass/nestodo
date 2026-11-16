import { Icon } from "./icons";
import { tokens } from "./title";

export function Tokens({ text, query, focusable = false }: { text: string; query: string; focusable?: boolean }) {
  return (
    <>
      {tokens(text, query).map((part, index) =>
        part.href ? (
          <a
            key={index}
            className={part.className}
            href={part.href}
            target="_blank"
            rel="noreferrer"
            tabIndex={focusable ? undefined : -1}
            data-from={part.from}
            data-to={part.to}
          >
            {part.text}
            <Icon name="link" size={12} strokeWidth={2.6} />
          </a>
        ) : (
          <span key={index} className={part.className} data-from={part.from} data-to={part.to}>
            {part.text}
          </span>
        ),
      )}
    </>
  );
}
