import { useEffect, useLayoutEffect, useRef, useState, type ClipboardEvent, type KeyboardEvent, type MouseEvent, type PointerEvent } from "react";
import { flushSync } from "react-dom";
import { AppHeader, useBackdropClose } from "./layout";
import { Icon } from "../src/icons";
import type { Props } from "./index.server";
import {
  ROOT,
  append,
  caretIn,
  destinations,
  fromTodos,
  guides,
  indent,
  keyFor,
  mergeIntoPrevious,
  moveBy,
  moveTo,
  openUnder,
  outdent,
  remove,
  rename,
  setCollapsed,
  setNote,
  split,
  subtree,
  toggleDone,
  visibleRows,
  type Change,
  type Focus,
  type Outline,
} from "../src/outline";
import { Sync } from "../src/sync";
import { encodePastedUrl, findLinks, tokens } from "../src/title";
import { MAX_NOTE_LENGTH, MAX_TITLE_LENGTH, SHOW_DONE_COOKIE } from "../src/todos";
import { UndoHistory } from "../src/undo";

function send(method: string, path: string, body?: unknown) {
  return fetch(path, {
    method,
    keepalive: true,
    headers: body === undefined ? { Accept: "application/json" } : { Accept: "application/json", "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
}

async function latestTodos() {
  const res = await send("GET", "/api/v1/todos");
  return res.ok ? ((await res.json()) as Pick<Props, "todos">).todos : null;
}

function caretAt(mirror: Element, x: number, title: string) {
  for (const part of mirror.children) {
    const box = part.getBoundingClientRect();
    if (x >= box.right || !(part instanceof HTMLElement)) continue;
    const from = Number(part.dataset.from);
    const to = Number(part.dataset.to);
    if (part instanceof HTMLAnchorElement) return x < (box.left + box.right) / 2 ? from : to;
    const text = part.firstChild;
    if (!text) return from;
    const range = document.createRange();
    let lo = 0;
    let hi = to - from;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      range.setStart(text, mid);
      range.setEnd(text, mid + 1);
      const char = range.getBoundingClientRect();
      if ((char.left + char.right) / 2 < x) lo = mid + 1;
      else hi = mid;
    }
    return from + lo;
  }
  return title.length;
}

function composing(event: Event) {
  return event instanceof InputEvent && event.isComposing;
}

function composingKey(event: globalThis.KeyboardEvent) {
  return event.isComposing || event.keyCode === 229;
}

function isUndo(event: Pick<KeyboardEvent, "key" | "metaKey" | "ctrlKey" | "shiftKey" | "altKey">) {
  return (event.key === "z" || event.key === "Z") && (event.metaKey || event.ctrlKey) && !event.shiftKey && !event.altKey;
}

function arrowStep(event: KeyboardEvent) {
  const k = event.key;
  if (k === "ArrowUp" || (event.ctrlKey && (k === "p" || k === "P"))) return -1;
  if (k === "ArrowDown" || (event.ctrlKey && (k === "n" || k === "N"))) return 1;
  return 0;
}

function rememberShowDone(shown: boolean) {
  document.cookie = `${SHOW_DONE_COOKIE}=${shown}; Path=/; Max-Age=${400 * 24 * 60 * 60}; SameSite=Lax`;
}

function fitNote(area: HTMLTextAreaElement) {
  area.style.height = "auto";
  area.style.height = `${area.scrollHeight}px`;
}

function Tokens({ text, query }: { text: string; query: string }) {
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
            tabIndex={-1}
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

function MovePicker({
  outline,
  moving,
  showDone,
  onMove,
  onClose,
}: {
  outline: Outline;
  moving: string;
  showDone: boolean;
  onMove: (target: string) => void;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const list = useRef<HTMLUListElement>(null);
  const backdrop = useBackdropClose();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const found = destinations(outline, moving, showDone, query);
  const selected = found[Math.min(active, found.length - 1)];
  const name = outline[moving].title || "empty item";

  useLayoutEffect(() => dialog.current?.showModal(), []);

  useLayoutEffect(() => {
    list.current?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
  }, [selected?.key]);

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (composingKey(event.nativeEvent)) return;
    const step = arrowStep(event);
    if (step !== 0 && selected) {
      event.preventDefault();
      const at = found.indexOf(selected) + step;
      setActive(Math.max(0, Math.min(found.length - 1, at)));
    } else if (event.key === "Enter" && selected) {
      event.preventDefault();
      onMove(selected.key);
    }
  }

  return (
    <dialog ref={dialog} className="move-to" aria-label={`Move "${name}" to`} onClose={onClose} {...backdrop}>
      <label className="search">
        <Icon name="filter" size={18} />
        <input
          role="combobox"
          aria-label={`Move "${name}" to`}
          aria-expanded="true"
          aria-controls="move-to-options"
          aria-autocomplete="list"
          aria-activedescendant={selected ? `move-to-${selected.key}` : undefined}
          placeholder={`Move "${name}" to…`}
          value={query}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => {
            setQuery(event.target.value);
            setActive(0);
          }}
          onKeyDown={onKeyDown}
        />
      </label>
      {found.length > 0 ? (
        <ul ref={list} id="move-to-options" role="listbox">
          {found.map((destination) => (
            <li
              key={destination.key}
              id={`move-to-${destination.key}`}
              role="option"
              aria-selected={destination === selected}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => onMove(destination.key)}
            >
              <span>
                {tokens(destination.title, query).map((part, index) => (
                  <span key={index} className={part.className}>
                    {part.text}
                  </span>
                ))}
              </span>
              {destination.path.length > 0 && <span className="move-to-path">{destination.path.join(" › ")}</span>}
            </li>
          ))}
        </ul>
      ) : (
        <p className="move-to-empty">No matches.</p>
      )}
    </dialog>
  );
}

type Confirming = { kind: "done" | "delete"; key: string; focus?: Focus };

function ConfirmDialog({
  outline,
  confirming: { kind, key },
  onConfirm,
  onClose,
}: {
  outline: Outline;
  confirming: Confirming;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const action = useRef<HTMLButtonElement>(null);
  const backdrop = useBackdropClose();
  const under = kind === "done" ? openUnder(outline, key) : subtree(outline, key).slice(1);
  const count = under.length;
  const items = count === 1 ? "item" : "items";
  const name = outline[key].title || "Untitled";
  const shown = under.slice(0, 3);

  useLayoutEffect(() => {
    dialog.current?.showModal();
    action.current?.focus();
  }, []);

  return (
    <dialog ref={dialog} className="confirm" aria-labelledby="confirm-title" onClose={onClose} {...backdrop}>
      <h2 id="confirm-title">
        {kind === "done" ? `Mark "${name}" as done?` : `Delete "${name}" and the ${count} ${items} under it?`}
      </h2>
      {kind === "done" && (
        <p>
          {count === 1
            ? "1 item under it isn't done yet. It'll be marked as done too."
            : `${count} items under it aren't done yet. They'll be marked as done too.`}
        </p>
      )}
      <ul className="confirm-items">
        {shown.map((item) => (
          <li key={item} className={outline[item].done ? "done" : undefined}>
            {outline[item].title || "Untitled"}
          </li>
        ))}
        {count > shown.length && <li className="more">and {count - shown.length} more</li>}
      </ul>
      <div className="confirm-actions">
        <button type="button" className="pill" onClick={() => dialog.current?.close()}>
          Cancel
        </button>
        <button ref={action} type="button" className={kind === "done" ? "pill primary" : "pill danger"} onClick={onConfirm}>
          {kind === "done" ? "Mark all as done" : "Delete all"}
        </button>
      </div>
    </dialog>
  );
}

export default function OutlinePage({ todos, showDone: startShown }: Props) {
  const [outline, setOutline] = useState(() => fromTodos(todos));
  const [showDone, setShowDone] = useState(startShown);
  const [query, setQuery] = useState("");
  const [filtering, setFiltering] = useState(false);
  const [kept, setKept] = useState<ReadonlySet<string>>(() => new Set());
  const [failed, setFailed] = useState(false);
  const [noteKey, setNoteKey] = useState<string | null>(null);
  const [moving, setMoving] = useState<Focus | null>(null);
  const [confirming, setConfirming] = useState<Confirming | null>(null);
  const [sync] = useState(
    () => new Sync(todos.map((todo) => [keyFor(todo.id), todo.id] as [string, number]), send, () => setFailed(true)),
  );
  const [history] = useState(() => new UndoHistory());
  const counter = useRef(0);
  const inputs = useRef(new Map<string, HTMLInputElement>());
  const notes = useRef(new Map<string, HTMLTextAreaElement>());
  const pendingFocus = useRef<Focus | null>(null);
  const clickCaret = useRef<number | null>(null);
  const refreshing = useRef(false);
  const reloading = useRef(false);
  const filterInput = useRef<HTMLInputElement>(null);
  const filterButton = useRef<HTMLButtonElement>(null);

  const rows = visibleRows(outline, showDone, query, kept);
  const lines = guides(rows);
  const shownKeys = new Set(rows.map((row) => row.key));
  const match = query.trim().toLowerCase();

  useEffect(() => {
    const refresh = async () => {
      if (refreshing.current || sync.pending || sync.failed) return;
      refreshing.current = true;
      const revision = sync.revision;
      const latest = await latestTodos().catch(() => null);
      refreshing.current = false;
      if (!latest || sync.revision !== revision || sync.pending || sync.failed) return;
      const keyOf = (id: number) => sync.keyOf(id) ?? keyFor(id);
      sync.track(latest.map((todo) => [keyOf(todo.id), todo.id]));
      setOutline(fromTodos(latest, keyOf));
    };
    const onBeforeUnload = (event: BeforeUnloadEvent) => {
      sync.flush();
      if (sync.pending && !reloading.current) event.preventDefault();
    };
    const onVisibilityChange = () => {
      if (document.visibilityState === "hidden") sync.flush();
      else void refresh();
    };
    const onFocus = () => void refresh();
    window.addEventListener("beforeunload", onBeforeUnload);
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      window.removeEventListener("beforeunload", onBeforeUnload);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [sync]);

  function fresh() {
    counter.current += 1;
    return `new${counter.current}`;
  }

  function focusRow({ key, pos, note }: Focus) {
    const input = note ? notes.current.get(key) : inputs.current.get(key);
    if (!input) return;
    input.focus();
    const at = caretIn(input.value, pos);
    input.setSelectionRange(at, at);
  }

  function focused(): Focus | undefined {
    for (const [key, input] of inputs.current) {
      if (input === document.activeElement) return { key, pos: input.selectionStart ?? 0 };
    }
    for (const [key, area] of notes.current) {
      if (area === document.activeElement) return { key, pos: area.selectionStart, note: true };
    }
  }

  useLayoutEffect(() => {
    const focus = pendingFocus.current;
    if (!focus) return;
    pendingFocus.current = null;
    focusRow(focus);
  });

  function apply(change: Change | null, focus?: Focus, before = focused()) {
    if (!change || sync.failed) return false;
    history.record(outline, change, before);
    commit(change, focus);
    return true;
  }

  function keep(keys: readonly string[]) {
    if (query) setKept((previous) => (keys.every((key) => previous.has(key)) ? previous : new Set([...previous, ...keys])));
  }

  function commit(change: Change, focus?: Focus) {
    setOutline(change.outline);
    sync.apply(change.ops);
    keep(change.ops.map((op) => op.key));
    const next = focus ?? change.focus ?? null;
    if (next?.note) setNoteKey(next.key);
    pendingFocus.current = next;
  }

  function reload() {
    reloading.current = true;
    window.location.reload();
  }

  function undo() {
    if (sync.failed) return;
    const change = history.undo(outline);
    if (change) commit(change);
  }

  function toggleShowDone() {
    const shown = !showDone;
    setShowDone(shown);
    rememberShowDone(shown);
  }

  function openFilter() {
    flushSync(() => setFiltering(true));
    filterInput.current?.focus();
  }

  function closeFilter() {
    flushSync(() => {
      setQuery("");
      setKept(new Set());
      setFiltering(false);
    });
    filterButton.current?.focus();
  }

  useEffect(() => {
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.altKey || composingKey(event)) return;
      const inDialog = event.target instanceof Element && event.target.closest("dialog") !== null;
      if ((event.key === "o" || event.key === "O") && (event.metaKey || event.ctrlKey) && !event.shiftKey) {
        event.preventDefault();
        if (!inDialog) toggleShowDone();
        return;
      }
      if (inDialog || event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return;
      if (isUndo(event)) {
        event.preventDefault();
        undo();
        return;
      }
      if (event.key !== "/" || event.metaKey || event.ctrlKey) return;
      event.preventDefault();
      openFilter();
    };
    const onBeforeInput = (event: InputEvent) => {
      if (event.inputType !== "historyUndo" || !(event.target instanceof Element && event.target.closest(".outline"))) return;
      event.preventDefault();
      undo();
    };
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("beforeinput", onBeforeInput);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("beforeinput", onBeforeInput);
    };
  });

  function needsConfirming(kind: Confirming["kind"], key: string) {
    if (sync.failed) return false;
    return kind === "done" ? !outline[key].done && openUnder(outline, key).length > 0 : outline[key].children.length > 0;
  }

  function complete(key: string, index: number, pos: number, confirmed = false) {
    if (!confirmed && needsConfirming("done", key)) return setConfirming({ kind: "done", key, focus: { key, pos } });
    const change = toggleDone(outline, key);
    if (showDone || !change.outline[key].done) return apply(change, { key, pos });
    const next = rows.slice(index + 1).find((row) => row.depth <= rows[index].depth) ?? rows[index - 1];
    apply(change, next ? { key: next.key, pos: -1 } : undefined);
  }

  function check(key: string) {
    if (needsConfirming("done", key)) return setConfirming({ kind: "done", key });
    apply(toggleDone(outline, key));
  }

  function removeRow(key: string, index: number, confirmed = false) {
    if (!confirmed && needsConfirming("delete", key)) return setConfirming({ kind: "delete", key, focus: focused() });
    const next = rows[index - 1] ?? rows.slice(index + 1).find((row) => row.depth <= rows[index].depth);
    apply(remove(outline, key), next ? { key: next.key, pos: -1 } : undefined);
  }

  function confirm() {
    if (!confirming) return;
    setConfirming(null);
    const { kind, key, focus } = confirming;
    if (!outline[key]) return;
    const index = rows.findIndex((row) => row.key === key);
    if (kind === "delete") removeRow(key, index, true);
    else if (focus) complete(key, index, focus.pos, true);
    else apply(toggleDone(outline, key));
  }

  function closeConfirm() {
    if (!confirming) return;
    setConfirming(null);
    if (confirming.focus) pendingFocus.current = confirming.focus;
  }

  function openMove(focus: Focus) {
    if (!sync.failed) setMoving(focus);
  }

  function moveRow(target: string) {
    if (!moving) return;
    setMoving(null);
    const index = rows.findIndex((row) => row.key === moving.key);
    const next = index < 0 ? undefined : (rows.slice(index + 1).find((row) => row.depth <= rows[index].depth) ?? rows[index - 1]);
    if (!apply(moveTo(outline, moving.key, target), next ? { key: next.key, pos: -1 } : undefined, moving)) pendingFocus.current = moving;
  }

  function closeMove() {
    if (!moving) return;
    setMoving(null);
    pendingFocus.current = moving;
  }

  function onTitle(key: string, title: string, composing = false) {
    const next = rename(outline, key, title);
    history.type(outline, next, key, "title", composing);
    setOutline(next);
    sync.title(key, title);
    keep([key]);
  }

  function onNote(key: string, note: string, composing = false) {
    const next = setNote(outline, key, note);
    history.type(outline, next, key, "note", composing);
    setOutline(next);
    sync.note(key, note);
    keep([key]);
  }

  function openNote(key: string) {
    flushSync(() => setNoteKey(key));
    const area = notes.current.get(key);
    if (!area) return;
    area.focus();
    area.setSelectionRange(area.value.length, area.value.length);
  }

  function closeNote(key: string) {
    const note = outline[key]?.note ?? "";
    if (note !== "" && note.trim() === "") onNote(key, "");
    history.end();
    sync.flush();
    setNoteKey((open) => (open === key ? null : open));
  }

  function onNoteClick(event: MouseEvent<HTMLElement>, key: string) {
    if ((event.target as Element).closest("a") || !window.getSelection()?.isCollapsed) return;
    focusRow({ key, pos: -1 });
  }

  function onPointerDown(event: PointerEvent<HTMLInputElement>, title: string) {
    const input = event.currentTarget;
    const mirror = input.nextElementSibling;
    const unfocused = document.activeElement !== input && mirror && findLinks(title).length > 0;
    clickCaret.current = unfocused ? caretAt(mirror, event.clientX, title) : null;
  }

  function onClick(event: MouseEvent<HTMLInputElement>) {
    const at = clickCaret.current;
    clickCaret.current = null;
    const input = event.currentTarget;
    if (at === null || input.selectionStart !== input.selectionEnd) return;
    const pos = caretIn(input.value, at);
    input.setSelectionRange(pos, pos);
  }

  function onPaste(event: ClipboardEvent<HTMLInputElement | HTMLTextAreaElement>, maxLength: number) {
    const pasted = event.clipboardData.getData("text/plain");
    const url = encodePastedUrl(pasted);
    if (url === null || url === pasted.trim()) return;
    event.preventDefault();
    const input = event.currentTarget;
    const replaced = (input.selectionEnd ?? 0) - (input.selectionStart ?? 0);
    if (input.value.length - replaced + url.length <= maxLength) document.execCommand("insertText", false, url);
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>, key: string) {
    if (composingKey(event.nativeEvent)) return;
    const input = event.currentTarget;
    const pos = input.selectionStart ?? 0;
    const atStart = pos === 0 && input.selectionEnd === 0;
    const mod = event.metaKey || event.ctrlKey;
    const index = rows.findIndex((row) => row.key === key);
    const here = { key, pos };
    const k = event.key;
    const handle = (action: () => unknown) => {
      event.preventDefault();
      action();
    };

    if (isUndo(event)) return handle(undo);
    if (k === " " && event.ctrlKey) return handle(() => apply(setCollapsed(outline, key, !outline[key].collapsed), here));
    if (k === "Backspace" && mod && event.shiftKey) return handle(() => removeRow(key, index));
    if ((k === "m" || k === "M") && mod && event.shiftKey) return handle(() => openMove(here));
    if (k === "Enter" && mod) return handle(() => complete(key, index, pos));
    if (k === "Enter" && event.shiftKey && !event.altKey) return handle(() => openNote(key));
    if (k === "Enter" && !event.shiftKey && !event.altKey) return handle(() => apply(split(outline, key, pos, fresh(), shownKeys)));
    if (k === "Tab") return handle(() => apply(event.shiftKey ? outdent(outline, key) : indent(outline, key, shownKeys), here));
    if ((k === "ArrowUp" || k === "ArrowDown") && event.shiftKey && (event.altKey || mod)) {
      return handle(() => apply(moveBy(outline, key, k === "ArrowUp" ? -1 : 1, shownKeys), here));
    }
    if ((k === "ArrowUp" || k === "ArrowDown") && mod) return handle(() => apply(setCollapsed(outline, key, k === "ArrowUp"), here));
    const step = arrowStep(event);
    if (step !== 0 && !event.shiftKey && rows[index + step]) return handle(() => focusRow({ key: rows[index + step].key, pos }));
    if (k === "Backspace" && atStart) {
      const item = outline[key];
      if (item.title === "" && item.note === "" && item.children.length === 0) return handle(() => removeRow(key, index));
      const merged = mergeIntoPrevious(outline, rows, key);
      if (merged) return handle(() => apply(merged));
    }
  }

  function onNoteKeyDown(event: KeyboardEvent<HTMLTextAreaElement>, key: string) {
    if (composingKey(event.nativeEvent)) return;
    const area = event.currentTarget;
    const caret = area.selectionStart === area.selectionEnd ? area.selectionStart : null;
    const mod = event.metaKey || event.ctrlKey;
    const index = rows.findIndex((row) => row.key === key);
    const k = event.key;
    const handle = (action: () => unknown) => {
      event.preventDefault();
      action();
    };

    if (isUndo(event)) return handle(undo);
    if (k === "Escape" || (k === "Enter" && event.shiftKey && !mod && !event.altKey)) return handle(() => focusRow({ key, pos: -1 }));
    if ((k === "m" || k === "M") && mod && event.shiftKey) return handle(() => openMove({ key, pos: -1 }));
    if (k === "Enter" && mod) return handle(() => complete(key, index, -1));
    const step = arrowStep(event);
    if (step === -1 && !event.shiftKey && caret === 0) return handle(() => focusRow({ key, pos: -1 }));
    if (step === 1 && !event.shiftKey && caret === area.value.length && index < rows.length - 1) {
      return handle(() => focusRow({ key: rows[index + 1].key, pos: 0 }));
    }
  }

  return (
    <>
      <AppHeader>
        <div className="outline-tools">
          {filtering ? (
            <label
              className="search"
              onBlur={(event) => {
                if (!query && !event.currentTarget.contains(event.relatedTarget)) setFiltering(false);
              }}
            >
              <Icon name="filter" size={18} />
              <input
                ref={filterInput}
                type="search"
                aria-label="Filter"
                placeholder="Filter"
                value={query}
                onChange={(event) => {
                  setQuery(event.target.value);
                  setKept(new Set());
                }}
                onKeyDown={(event) => {
                  if (composingKey(event.nativeEvent) || event.key !== "Escape") return;
                  event.preventDefault();
                  closeFilter();
                }}
              />
              <button type="button" className="icon-button" aria-label="Close filter" onClick={closeFilter}>
                <Icon name="close" size={16} />
              </button>
            </label>
          ) : (
            <button ref={filterButton} type="button" className="icon-button" aria-label="Filter" data-tip="Filter (/)" onClick={openFilter}>
              <Icon name="filter" size={20} />
            </button>
          )}
          <button
            type="button"
            className="icon-button"
            aria-label="Show completed"
            aria-pressed={showDone}
            data-tip="Show completed (Ctrl/⌘ O)"
            onClick={toggleShowDone}
          >
            <Icon name="completed" size={20} />
          </button>
        </div>
      </AppHeader>

      <main className="outline-page">
        {failed && (
          <p role="alert" className="sync-error">
            Couldn't save your last change, so editing is paused.
            <button type="button" className="text-button" onClick={reload}>
              Reload
            </button>
          </p>
        )}

        {query && (
          <p className="filter-note">
            Matches are shown with their parents.
            <button type="button" className="text-button" onClick={closeFilter}>
              Clear filter
            </button>
          </p>
        )}

        <ul className="outline">
          {rows.map((row, index) => {
            const item = outline[row.key];
            const name = item.title || "empty item";
            const hasChildren = item.children.length > 0;
            const { through, stem } = lines[index];
            const toggle = () => apply(setCollapsed(outline, row.key, !item.collapsed));
            return (
              <li key={row.key} className={["row", hasChildren && "parent", item.done ? "done" : row.under && "under"].filter(Boolean).join(" ")}>
                {through.map((continues, level) => (
                  <span key={level} className={["guide", continues && "continues", level === row.depth - 1 && "branch"].filter(Boolean).join(" ")} aria-hidden="true" />
                ))}
                <span className={stem ? "marker stem" : "marker"}>
                  {hasChildren && (
                    <button
                      type="button"
                      className="fold"
                      aria-label={`${item.collapsed ? "Expand" : "Collapse"} ${name}`}
                      aria-expanded={!item.collapsed}
                      onClick={toggle}
                    >
                      <Icon name="expand" size={16} strokeWidth={2.2} />
                    </button>
                  )}
                  <button
                    type="button"
                    className={hasChildren && item.collapsed ? "bullet halo" : "bullet"}
                    tabIndex={-1}
                    aria-hidden="true"
                    disabled={!hasChildren}
                    onClick={toggle}
                  >
                    {item.done ? (
                      <span className="done-box">
                        <Icon name="tick" size={11} strokeWidth={3.6} />
                      </span>
                    ) : (
                      <span className="dot" />
                    )}
                  </button>
                </span>
                <span className="title">
                  <span className="line">
                    <input
                      ref={(input) => {
                        if (input) inputs.current.set(row.key, input);
                        else inputs.current.delete(row.key);
                      }}
                      value={item.title}
                      readOnly={failed}
                      aria-label={`${name}, level ${row.depth + 1}`}
                      maxLength={MAX_TITLE_LENGTH}
                      autoComplete="off"
                      spellCheck={false}
                      onChange={(event) => onTitle(row.key, event.target.value, composing(event.nativeEvent))}
                      onCompositionEnd={() => history.composed()}
                      onKeyDown={(event) => onKeyDown(event, row.key)}
                      onPointerDown={(event) => onPointerDown(event, item.title)}
                      onClick={onClick}
                      onPaste={(event) => onPaste(event, MAX_TITLE_LENGTH)}
                      onBlur={() => {
                        history.end();
                        sync.flush();
                      }}
                    />
                    <span className="mirror" aria-hidden="true">
                      <Tokens text={item.title} query={query} />
                    </span>
                  </span>
                  {noteKey === row.key ? (
                    <>
                      <textarea
                        ref={(area) => {
                          if (!area) {
                            notes.current.delete(row.key);
                            return;
                          }
                          notes.current.set(row.key, area);
                          fitNote(area);
                        }}
                        className="note-input"
                        value={item.note}
                        readOnly={failed}
                        rows={1}
                        aria-label={`Note of ${name}`}
                        maxLength={MAX_NOTE_LENGTH}
                        spellCheck={false}
                        onChange={(event) => onNote(row.key, event.target.value, composing(event.nativeEvent))}
                        onCompositionEnd={() => history.composed()}
                        onKeyDown={(event) => onNoteKeyDown(event, row.key)}
                        onPaste={(event) => onPaste(event, MAX_NOTE_LENGTH)}
                        onBlur={() => closeNote(row.key)}
                      />
                      <span className="note-hint" aria-hidden="true">
                        <kbd>Enter</kbd> new line · <kbd>⇧</kbd>
                        <kbd>Enter</kbd> or <kbd>Esc</kbd> back to the title
                      </span>
                    </>
                  ) : (
                    item.note !== "" && (
                      <span
                        className={match && item.note.toLowerCase().includes(match) ? "note whole" : "note"}
                        onClick={(event) => onNoteClick(event, row.key)}
                      >
                        <Tokens text={item.note} query={query} />
                      </span>
                    )
                  )}
                </span>
                <button
                  type="button"
                  className="note-button"
                  aria-label={`${item.note === "" ? "Add a note to" : "Edit the note of"} ${name}`}
                  data-tip="Note (⇧ Enter)"
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => (noteKey === row.key ? focusRow({ key: row.key, pos: -1 }) : openNote(row.key))}
                >
                  <Icon name="note" size={18} />
                </button>
                <button
                  type="button"
                  className="check"
                  aria-label={`Mark ${name} as ${item.done ? "not done" : "done"}`}
                  aria-pressed={item.done}
                  onClick={() => check(row.key)}
                >
                  <Icon name={item.done ? "completed" : "done"} size={18} />
                </button>
              </li>
            );
          })}
        </ul>

        {query && rows.length === 0 && <p className="empty">No matches.</p>}

        <button type="button" className="add-item" onClick={() => apply(append(outline, ROOT, fresh()))}>
          <Icon name="add" size={18} />
          Add item
        </button>
      </main>

      {moving && outline[moving.key] && (
        <MovePicker outline={outline} moving={moving.key} showDone={showDone} onMove={moveRow} onClose={closeMove} />
      )}
      {confirming && outline[confirming.key] && (
        <ConfirmDialog outline={outline} confirming={confirming} onConfirm={confirm} onClose={closeConfirm} />
      )}
    </>
  );
}
