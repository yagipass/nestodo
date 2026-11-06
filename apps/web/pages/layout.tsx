import { useEffect, useRef, useState, type ComponentProps, type MouseEvent, type ReactNode, type RefObject } from "react";
import { Link, useRouter, useShared } from "@void/react";
import { startAnalytics } from "../src/analytics";
import "../src/styles.css";

type Shortcut = [action: string, keys: string, note?: string];

const SHORTCUTS: { name: string; shortcuts: Shortcut[] }[] = [
  {
    name: "Editing",
    shortcuts: [
      ["New item", "Enter"],
      ["Complete / reopen", "Ctrl/⌘ Enter"],
      ["Join with item above", "⌫", "at start"],
      ["Delete with children", "Ctrl/⌘ ⇧ ⌫"],
      ["Undo", "Ctrl/⌘ Z"],
    ],
  },
  {
    name: "Notes",
    shortcuts: [
      ["Write a note under the item", "⇧ Enter"],
      ["Back to the title", "⇧ Enter / Esc", "in a note"],
      ["New line", "Enter", "in a note"],
    ],
  },
  {
    name: "Structure",
    shortcuts: [
      ["Indent / outdent", "Tab / ⇧ Tab"],
      ["Move up / down", "Alt ⇧ ↑ / ↓"],
      ["Move up / down", "Ctrl/⌘ ⇧ ↑ / ↓"],
      ["Move under another item", "Ctrl/⌘ ⇧ M"],
      ["Collapse / expand", "Ctrl/⌘ ↑ / ↓"],
      ["Toggle collapse", "Ctrl Space"],
    ],
  },
  {
    name: "Navigation & view",
    shortcuts: [
      ["Previous / next item", "↑ / ↓"],
      ["Previous / next item", "Ctrl P / Ctrl N"],
      ["Show / hide completed", "Ctrl/⌘ O"],
      ["Filter", "/", "outside an item"],
    ],
  },
];

function isOnBackdrop(event: MouseEvent<HTMLDialogElement>) {
  if (event.target !== event.currentTarget) return false;
  const box = event.currentTarget.getBoundingClientRect();
  return event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom;
}

export function useBackdropClose(): Pick<ComponentProps<"dialog">, "onPointerDown" | "onClick"> {
  const pressedBackdrop = useRef(false);
  return {
    onPointerDown: (event) => {
      pressedBackdrop.current = isOnBackdrop(event);
    },
    onClick: (event) => {
      if (pressedBackdrop.current && isOnBackdrop(event)) event.currentTarget.close();
    },
  };
}

function ShortcutsDialog({ ref, onClose }: { ref: RefObject<HTMLDialogElement | null>; onClose: () => void }) {
  const backdrop = useBackdropClose();

  return (
    <dialog ref={ref} className="shortcuts" aria-labelledby="shortcuts-title" onClose={onClose} {...backdrop}>
      <div className="shortcuts-head">
        <h2 id="shortcuts-title">Keyboard shortcuts</h2>
        <button type="button" className="icon-button" aria-label="Close" onClick={() => ref.current?.close()}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
            <path d="M6 6l12 12M18 6L6 18" />
          </svg>
        </button>
      </div>
      <div className="shortcuts-groups">
        {SHORTCUTS.map(({ name, shortcuts }) => (
          <section key={name}>
            <h3>{name}</h3>
            <ul>
              {shortcuts.map(([action, keys, note]) => (
                <li key={keys}>
                  {action}
                  <span className="shortcut-keys">
                    {keys.split(" ").map((key, index) => (key === "/" && index > 0 ? <span key={index}>/</span> : <kbd key={index}>{key}</kbd>))}
                    {note && <span>{note}</span>}
                  </span>
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>
    </dialog>
  );
}

function AccountMenu({ email }: { email: string }) {
  const { path } = useRouter();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const shortcuts = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setOpen(false);
      button.current?.focus();
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  return (
    <div className="account" ref={root}>
      <button
        ref={button}
        type="button"
        className="icon-button"
        aria-label="Account"
        aria-expanded={open}
        aria-controls="account-menu"
        onClick={() => setOpen(!open)}
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z" />
        </svg>
      </button>
      <div id="account-menu" className="account-menu" hidden={!open}>
        <p className="account-email">{email}</p>
        <hr />
        <Link href="/stats" aria-current={path === "/stats" ? "page" : undefined} onClick={() => setOpen(false)}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
            <path d="M5 20v-8M12 20V4M19 20V9" />
          </svg>
          Stats
        </Link>
        <Link href="/settings" aria-current={path === "/settings" ? "page" : undefined} onClick={() => setOpen(false)}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
            <path d="M4 7h10M18 7h2M4 17h2M10 17h10" />
            <circle cx="16" cy="7" r="2" />
            <circle cx="8" cy="17" r="2" />
          </svg>
          Settings
        </Link>
        <button
          type="button"
          aria-haspopup="dialog"
          onClick={() => {
            setOpen(false);
            shortcuts.current?.showModal();
          }}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <circle cx="12" cy="12" r="9" />
            <path d="M9.6 9.2a2.5 2.5 0 1 1 3.4 2.3c-.6.3-1 .9-1 1.6v.4M12 17h.01" />
          </svg>
          Help
        </button>
        <hr />
        <a href="/cdn-cgi/access/logout">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M9 4H5a1 1 0 0 0-1 1v14a1 1 0 0 0 1 1h4M16 17l5-5-5-5M21 12H9" />
          </svg>
          Sign out
        </a>
      </div>
      <ShortcutsDialog ref={shortcuts} onClose={() => button.current?.focus()} />
    </div>
  );
}

export function AppHeader({ children }: { children?: ReactNode }) {
  const user = useShared()?.auth.user ?? null;

  return (
    <header className="app-header">
      <Link href="/" className="brand">
        Nestodo
      </Link>
      {children}
      {user && <AccountMenu email={user.email} />}
    </header>
  );
}

export default function Layout({ children }: { children: ReactNode }) {
  useEffect(() => {
    void startAnalytics();
  }, []);
  return <div className="app">{children}</div>;
}
