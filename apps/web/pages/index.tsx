import { useEffect, useLayoutEffect, useRef, useState, type ClipboardEvent, type KeyboardEvent, type MouseEvent, type PointerEvent } from "react";
import { flushSync } from "react-dom";
import { useShared } from "@void/react";
import { AppHeader, Avatar, useBackdropClose } from "./layout";
import { base64Of, squareImageFrom } from "../src/avatar-image";
import { Icon } from "../src/icons";
import { IMAGE_ICON_SIZE, nameFromFile, type ImageIcon } from "../src/image-icons";
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
  nextHit,
  openUnder,
  outdent,
  parentOf,
  remove,
  rename,
  searchHits,
  setCollapsed,
  setComments,
  setIcon,
  split,
  subtree,
  toggleDone,
  visibleRows,
  withoutIcon,
  type Change,
  type Focus,
  type Outline,
} from "../src/outline";
import { Sync } from "../src/sync";
import { ThreadPane } from "../src/thread-pane";
import { encodePastedUrl, findLinks, tokens } from "../src/title";
import { MAX_TITLE_LENGTH, SHOW_DONE_COOKIE } from "../src/todos";
import { ICON_CHOICES, imageIcon, isIcon, shownIcon, type ShownIcon } from "../src/todo-icon";
import { Tokens } from "../src/tokens";
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

async function latestImages() {
  const res = await send("GET", "/api/v1/icons");
  return res.ok ? ((await res.json()) as { icons: ImageIcon[] }).icons : null;
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

function TodoIconGlyph({ icon }: { icon: ShownIcon }) {
  return "emoji" in icon ? icon.emoji : <img src={icon.image.url} alt="" />;
}

function DoneBadge() {
  return (
    <span className="done-badge">
      <Icon name="tick" size={8} strokeWidth={4} />
    </span>
  );
}

function IconPreviewRow({ title, image, done = false }: { title: string; image: string; done?: boolean }) {
  return (
    <div className={done ? "icon-preview-row done" : "icon-preview-row"}>
      <span className="icon-preview-mark">
        <img src={image} alt="" />
        {done && <DoneBadge />}
      </span>
      <span>{title}</span>
    </div>
  );
}

type ImageView =
  | { kind: "add"; image: string; preview: string; name: string }
  | { kind: "edit"; image: ImageIcon; name: string }
  | { kind: "delete"; image: ImageIcon };

async function problemOf(res: Response | null, fallback: string) {
  const body = (await res?.json().catch(() => null)) as { message?: unknown; issues?: { issues: { message: string }[] }[] } | null | undefined;
  if (typeof body?.message === "string") return body.message;
  return body?.issues?.flatMap(({ issues }) => issues)[0]?.message ?? fallback;
}

function IconPicker({
  outline,
  picking,
  images,
  onPick,
  onAdded,
  onRenamed,
  onDeleted,
  onClose,
}: {
  outline: Outline;
  picking: string;
  images: readonly ImageIcon[];
  onPick: (icon: string | null) => void;
  onAdded: (image: ImageIcon) => void;
  onRenamed: (image: ImageIcon) => void;
  onDeleted: (id: string) => void;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const choices = useRef<HTMLDivElement>(null);
  const file = useRef<HTMLInputElement>(null);
  const field = useRef<HTMLInputElement>(null);
  const action = useRef<HTMLButtonElement>(null);
  const backdrop = useBackdropClose();
  const [other, setOther] = useState("");
  const [refused, setRefused] = useState(false);
  const [view, setView] = useState<ImageView | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const open = useRef(true);
  const latest = useRef({ onPick, onAdded, onRenamed, onDeleted });
  const { title, icon } = outline[picking];
  const name = title || "empty item";
  const preview = view?.kind === "add" ? view.preview : null;

  useLayoutEffect(() => {
    dialog.current?.showModal();
  }, []);

  useLayoutEffect(() => {
    if (view === null) {
      (dialog.current?.querySelector<HTMLButtonElement>('.icon-choices [aria-pressed="true"]') ?? choices.current?.querySelector("button"))?.focus();
    } else if (view.kind === "delete") {
      action.current?.focus();
    } else {
      field.current?.focus();
      field.current?.select();
    }
  }, [view?.kind]);

  useEffect(() => () => void (preview && URL.revokeObjectURL(preview)), [preview]);

  useLayoutEffect(() => {
    latest.current = { onPick, onAdded, onRenamed, onDeleted };
  });

  useEffect(() => {
    open.current = true;
    return () => {
      open.current = false;
    };
  }, []);

  function show(next: ImageView | null) {
    setProblem(null);
    setView(next);
  }

  function back() {
    show(view?.kind === "delete" ? { kind: "edit", image: view.image, name: view.image.name } : null);
  }

  async function choose(chosen: File | undefined) {
    if (file.current) file.current.value = "";
    if (!chosen) return;
    setProblem(null);
    const square = await squareImageFrom(chosen, IMAGE_ICON_SIZE).catch(() => null);
    const image = square && (await base64Of(square).catch(() => null));
    if (!square || !image) {
      setProblem("The image could not be read.");
      return;
    }
    show({ kind: "add", image, preview: URL.createObjectURL(square), name: nameFromFile(chosen.name) });
  }

  async function request(method: string, path: string, body: unknown, fallback: string, alsoDone?: number) {
    setBusy(true);
    setProblem(null);
    const res = await fetch(path, {
      method,
      headers: { Accept: "application/json", "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    }).catch(() => null);
    setBusy(false);
    if (res && (res.ok || res.status === alsoDone)) return res;
    setProblem(await problemOf(res, fallback));
    return null;
  }

  function nameIn(value: string) {
    const trimmed = value.normalize("NFC").trim();
    if (trimmed === "") setProblem("Enter a name");
    return trimmed === "" ? null : trimmed;
  }

  async function save() {
    if (busy || view === null || view.kind === "delete") return;
    const imageName = nameIn(view.name);
    if (imageName === null) return;
    if (view.kind === "add") {
      const res = await request("POST", "/api/v1/icons", { name: imageName, image: view.image }, "Could not add the image.");
      if (!res) return;
      const { icon: added } = (await res.json()) as { icon: ImageIcon };
      latest.current.onAdded(added);
      if (open.current) latest.current.onPick(imageIcon(added.id));
      return;
    }
    const res = await request("PATCH", `/api/v1/icons/${view.image.id}`, { name: imageName }, "Could not save the image.");
    if (!res) return;
    latest.current.onRenamed(((await res.json()) as { icon: ImageIcon }).icon);
    show(null);
  }

  async function remove() {
    if (busy || view?.kind !== "delete") return;
    if (!(await request("DELETE", `/api/v1/icons/${view.image.id}`, undefined, "Could not delete the image.", 404))) return;
    latest.current.onDeleted(view.image.id);
    show(null);
  }

  function onOtherKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key !== "Enter" || composingKey(event.nativeEvent)) return;
    event.preventDefault();
    const value = other.trim();
    if (isIcon(value)) onPick(value);
    else setRefused(true);
  }

  const problemNote = problem && (
    <p id="icon-image-problem" className="icon-other-error" role="alert">
      {problem}
    </p>
  );

  if (view?.kind === "delete") {
    const used = Object.values(outline).filter((item) => item.icon === imageIcon(view.image.id)).length;
    return (
      <dialog ref={dialog} className="icon-picker" aria-label={`Icon for "${name}"`} onClose={onClose} {...backdrop}>
        <div className="icon-form-head">
          <button type="button" className="icon-button" aria-label="Back" onClick={back}>
            <Icon name="back" size={18} />
          </button>
          <h2>Delete "{view.image.name}"?</h2>
        </div>
        <p className="icon-form-note">
          {used === 0
            ? "Deleting it cannot be undone."
            : used === 1
              ? "It is the icon of 1 todo. Deleting it removes the icon from that todo and cannot be undone."
              : `It is the icon of ${used} todos. Deleting it removes the icon from them and cannot be undone.`}
        </p>
        {problemNote}
        <div className="icon-form-actions">
          <button type="button" className="pill" onClick={back}>
            Back
          </button>
          <button ref={action} type="button" className="pill danger" disabled={busy} onClick={() => void remove()}>
            Delete
          </button>
        </div>
      </dialog>
    );
  }

  if (view !== null) {
    const shown = view.kind === "add" ? view.preview : view.image.url;
    return (
      <dialog ref={dialog} className="icon-picker" aria-label={`Icon for "${name}"`} onClose={onClose} {...backdrop}>
        <form
          className="icon-form"
          onSubmit={(event) => {
            event.preventDefault();
            void save();
          }}
        >
          <div className="icon-form-head">
            <button type="button" className="icon-button" aria-label="Back" onClick={back}>
              <Icon name="back" size={18} />
            </button>
            <h2>{view.kind === "add" ? "Add image" : "Edit image"}</h2>
          </div>
          <div className="icon-preview">
            <img className="icon-preview-image" src={shown} alt="" />
            {view.kind === "add" && (
              <div className="icon-preview-rows">
                <p className="icon-section">In the outline</p>
                <IconPreviewRow title={title || "Untitled"} image={shown} />
                <IconPreviewRow title={title || "Untitled"} image={shown} done />
              </div>
            )}
          </div>
          <label className="icon-section" htmlFor="icon-image-name">
            Name
          </label>
          <input
            ref={field}
            id="icon-image-name"
            className="icon-other"
            value={view.name}
            autoComplete="off"
            spellCheck={false}
            aria-invalid={problem !== null}
            aria-describedby={problem ? "icon-image-problem" : undefined}
            onChange={(event) => {
              setProblem(null);
              setView({ ...view, name: event.target.value });
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter" && composingKey(event.nativeEvent)) event.preventDefault();
            }}
          />
          {problemNote}
          <div className="icon-form-actions">
            {view.kind === "edit" ? (
              <button type="button" className="text-button icon-delete" onClick={() => show({ kind: "delete", image: view.image })}>
                Delete image
              </button>
            ) : (
              <button type="button" className="pill" onClick={back}>
                Back
              </button>
            )}
            <button type="submit" className="pill primary" disabled={busy}>
              {view.kind === "add" ? "Add and use" : "Save"}
            </button>
          </div>
        </form>
      </dialog>
    );
  }

  return (
    <dialog ref={dialog} className="icon-picker" aria-label={`Icon for "${name}"`} onClose={onClose} {...backdrop}>
      <p id="icon-images-label" className="icon-section">
        My icons
      </p>
      <div className="icon-choices" role="group" aria-labelledby="icon-images-label">
        {images.map((image) => (
          <button
            key={image.id}
            type="button"
            title={image.name}
            aria-label={image.name}
            aria-pressed={icon === imageIcon(image.id)}
            onClick={() => onPick(imageIcon(image.id))}
            onContextMenu={(event) => {
              event.preventDefault();
              show({ kind: "edit", image, name: image.name });
            }}
          >
            <img src={image.url} alt="" />
          </button>
        ))}
        <button type="button" className="icon-add" title="Add image" aria-label="Add image" onClick={() => file.current?.click()}>
          <Icon name="add" size={18} />
        </button>
      </div>
      <input ref={file} type="file" accept="image/*" hidden aria-label="Image" onChange={(event) => void choose(event.target.files?.[0])} />
      {problemNote}
      <p id="icon-emoji-label" className="icon-section">
        Emoji
      </p>
      <div ref={choices} className="icon-choices" role="group" aria-labelledby="icon-emoji-label">
        {ICON_CHOICES.map((choice) => (
          <button key={choice} type="button" aria-pressed={choice === icon} onClick={() => onPick(choice)}>
            {choice}
          </button>
        ))}
      </div>
      <input
        className="icon-other"
        aria-label="Other emoji"
        aria-invalid={refused}
        aria-describedby={refused ? "icon-other-error" : undefined}
        placeholder="Other emoji, then Enter"
        value={other}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => {
          setOther(event.target.value);
          setRefused(false);
        }}
        onKeyDown={onOtherKeyDown}
      />
      {refused && (
        <p id="icon-other-error" className="icon-other-error" role="alert">
          Enter one emoji
        </p>
      )}
      {icon !== null && (
        <button type="button" className="text-button icon-remove" onClick={() => onPick(null)}>
          Remove icon
        </button>
      )}
    </dialog>
  );
}

type Confirming = { kind: "done" | "delete"; key: string; focus?: Focus; before?: Focus };

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

export default function OutlinePage({ todos, icons, showDone: startShown }: Props) {
  const [outline, setOutline] = useState(() => fromTodos(todos));
  const [images, setImages] = useState<readonly ImageIcon[]>(icons);
  const [showDone, setShowDone] = useState(startShown);
  const [query, setQuery] = useState("");
  const [searching, setSearching] = useState(false);
  const [current, setCurrent] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const [thread, setThread] = useState<string | null>(null);
  const [threadFocus, setThreadFocus] = useState<{ key: string; request: number } | null>(null);
  const [drafts, setDrafts] = useState<ReadonlyMap<string, string>>(() => new Map());
  const [, setTracked] = useState(0);
  const [moving, setMoving] = useState<Focus | null>(null);
  const [picking, setPicking] = useState<Focus | null>(null);
  const [confirming, setConfirming] = useState<Confirming | null>(null);
  const [sync] = useState(
    () =>
      new Sync(
        todos.map((todo) => [keyFor(todo.id), todo.id] as [string, number]),
        send,
        () => setFailed(true),
        () => setTracked((n) => n + 1),
      ),
  );
  const [history] = useState(() => new UndoHistory());
  const counter = useRef(0);
  const inputs = useRef(new Map<string, HTMLInputElement>());
  const pendingFocus = useRef<Focus | null>(null);
  const clickCaret = useRef<number | null>(null);
  const refreshing = useRef(false);
  const reloading = useRef(false);
  const searchInput = useRef<HTMLInputElement>(null);
  const searchButton = useRef<HTMLButtonElement>(null);

  const rows = visibleRows(outline, showDone);
  const lines = guides(rows);
  const shownKeys = new Set(rows.map((row) => row.key));
  const text = query.trim();
  const hits = searchHits(outline, rows, text);
  const at = current === null ? -1 : hits.indexOf(current);
  const threadItem = thread === null ? undefined : outline[thread];
  const avatarUrl = useShared()?.auth?.profile?.avatarUrl ?? null;

  useEffect(() => {
    const refresh = async () => {
      if (refreshing.current || sync.pending || sync.failed) return;
      refreshing.current = true;
      const revision = sync.revision;
      const [latest, latestIcons] = await Promise.all([latestTodos().catch(() => null), latestImages().catch(() => null)]);
      refreshing.current = false;
      if (latestIcons) setImages(latestIcons);
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

  function focusRow({ key, pos }: Focus) {
    const input = inputs.current.get(key);
    if (!input) return;
    input.focus();
    const at = caretIn(input.value, pos);
    input.setSelectionRange(at, at);
  }

  function focused(): Focus | undefined {
    for (const [key, input] of inputs.current) {
      if (input === document.activeElement) return { key, pos: input.selectionStart ?? 0 };
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

  function commit(change: Change, focus?: Focus) {
    setOutline(change.outline);
    sync.apply(change.ops);
    pendingFocus.current = focus ?? change.focus ?? null;
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

  function reveal(key: string) {
    inputs.current.get(key)?.scrollIntoView({ block: "nearest" });
  }

  function search(value: string) {
    setQuery(value);
    const found = searchHits(outline, rows, value);
    const next = current !== null && found.includes(current) ? current : nextHit(rows, found, current, 1);
    setCurrent(next);
    if (next !== null) reveal(next);
  }

  function openSearch() {
    flushSync(() => setSearching(true));
    searchInput.current?.focus();
  }

  function step(by: 1 | -1) {
    const next = nextHit(rows, hits, current, by);
    setCurrent(next);
    if (next !== null) reveal(next);
  }

  function closeSearch() {
    const found = at < 0 ? null : current;
    flushSync(() => {
      setQuery("");
      setCurrent(null);
      setSearching(false);
    });
    if (found === null) return searchButton.current?.focus();
    focusRow({ key: found, pos: outline[found].title.toLowerCase().indexOf(text.toLowerCase()) });
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
      openSearch();
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

  function complete(key: string, index: number, pos: number, confirmed = false, before = focused()) {
    if (!confirmed && needsConfirming("done", key)) return setConfirming({ kind: "done", key, focus: { key, pos }, before });
    const change = toggleDone(outline, key);
    if (showDone || !change.outline[key].done) return apply(change, { key, pos }, before);
    const next = rows.slice(index + 1).find((row) => row.depth <= rows[index].depth) ?? rows[index - 1];
    apply(change, next ? { key: next.key, pos: -1 } : undefined, before);
  }

  function check(key: string) {
    if (needsConfirming("done", key)) return setConfirming({ kind: "done", key, before: focused() });
    apply(toggleDone(outline, key));
  }

  function removeRow(key: string, index: number, confirmed = false, before = focused()) {
    if (!confirmed && needsConfirming("delete", key)) return setConfirming({ kind: "delete", key, focus: before, before });
    const next = rows[index - 1] ?? rows.slice(index + 1).find((row) => row.depth <= rows[index].depth);
    apply(remove(outline, key), next ? { key: next.key, pos: -1 } : undefined, before);
  }

  function confirm() {
    if (!confirming) return;
    setConfirming(null);
    const { kind, key, focus, before } = confirming;
    const item = outline[key];
    if (!item) return;
    if (kind === "done" && item.done) {
      if (focus) pendingFocus.current = focus;
      return;
    }
    const index = rows.findIndex((row) => row.key === key);
    if (index < 0) apply(kind === "done" ? toggleDone(outline, key) : remove(outline, key), undefined, before);
    else if (kind === "delete") removeRow(key, index, true, before);
    else if (focus) complete(key, index, focus.pos, true, before);
    else apply(toggleDone(outline, key), undefined, before);
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

  function openIcon(key: string) {
    const here = focused();
    if (!sync.failed) setPicking(here?.key === key ? here : { key, pos: -1 });
  }

  function pickIcon(icon: string | null) {
    if (!picking) return;
    setPicking(null);
    if (!outline[picking.key] || !apply(setIcon(outline, picking.key, icon), picking, picking)) pendingFocus.current = picking;
  }

  function addedImage(image: ImageIcon) {
    setImages((current) => (current.some((candidate) => candidate.id === image.id) ? current : [...current, image]));
  }

  function renamedImage(image: ImageIcon) {
    setImages((current) => current.map((candidate) => (candidate.id === image.id ? image : candidate)));
  }

  function deletedImage(id: string) {
    setImages((current) => current.filter((candidate) => candidate.id !== id));
    setOutline((current) => withoutIcon(current, imageIcon(id)));
    history.clear();
  }

  function closeIcon() {
    if (!picking) return;
    setPicking(null);
    pendingFocus.current = picking;
  }

  function onTitle(key: string, title: string, composing = false) {
    const next = rename(outline, key, title);
    history.type(outline, next, key, composing);
    setOutline(next);
    sync.title(key, title);
  }

  function openThread(key: string) {
    setThread(key);
    setThreadFocus((current) => ({ key, request: (current?.request ?? 0) + 1 }));
  }

  function followCaret(key: string) {
    if (thread === null || thread === key) return;
    setThread(key);
    setThreadFocus(null);
  }

  function editDraft(key: string, update: (current: string) => string) {
    setDrafts((current) => {
      const text = update(current.get(key) ?? "");
      const next = new Map(current);
      if (text === "") next.delete(key);
      else next.set(key, text);
      return next;
    });
  }

  function countComments(key: string, delta: number) {
    setOutline((current) => (current[key] ? setComments(current, key, Math.max(0, current[key].comments + delta)) : current));
  }

  function pathOf(key: string) {
    const path: string[] = [];
    for (let parent = parentOf(outline, key); parent !== null && parent !== ROOT; parent = parentOf(outline, parent)) {
      path.unshift(outline[parent].title || "Untitled");
    }
    return path;
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
    if ((k === "e" || k === "E") && mod && event.shiftKey) return handle(() => openIcon(key));
    if (k === "Enter" && mod) return handle(() => complete(key, index, pos));
    if (k === "Enter" && event.shiftKey && !event.altKey) return handle(() => openThread(key));
    if (k === "Escape" && thread !== null && !mod && !event.shiftKey) return handle(() => setThread(null));
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
      if (item.title === "" && item.comments === 0 && item.icon === null && item.children.length === 0) return handle(() => removeRow(key, index));
      const merged = mergeIntoPrevious(outline, rows, key);
      if (merged) return handle(() => apply(merged));
    }
  }

  return (
    <>
      <AppHeader>
        <div className="outline-tools">
          {searching ? (
            <label
              className="search"
              onBlur={(event) => {
                if (!query && !event.currentTarget.contains(event.relatedTarget)) setSearching(false);
              }}
            >
              <Icon name="filter" size={18} />
              <input
                ref={searchInput}
                type="search"
                aria-label="Search"
                placeholder="Search"
                value={query}
                onChange={(event) => search(event.target.value)}
                onKeyDown={(event) => {
                  if (composingKey(event.nativeEvent) || event.altKey || event.metaKey || event.ctrlKey) return;
                  if (event.key === "Enter") {
                    event.preventDefault();
                    step(event.shiftKey ? -1 : 1);
                  } else if (event.key === "Escape") {
                    event.preventDefault();
                    closeSearch();
                  }
                }}
              />
              {text && (
                <>
                  <span className="search-count" aria-live="polite">
                    {at + 1} / {hits.length}
                  </span>
                  <button
                    type="button"
                    className="icon-button"
                    aria-label="Previous match"
                    data-tip="Previous match (⇧ Enter)"
                    disabled={hits.length === 0}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => step(-1)}
                  >
                    <Icon name="up" size={16} />
                  </button>
                  <button
                    type="button"
                    className="icon-button"
                    aria-label="Next match"
                    data-tip="Next match (Enter)"
                    disabled={hits.length === 0}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => step(1)}
                  >
                    <Icon name="down" size={16} />
                  </button>
                </>
              )}
              <button type="button" className="icon-button" aria-label="Close search" onClick={closeSearch}>
                <Icon name="close" size={16} />
              </button>
            </label>
          ) : (
            <button ref={searchButton} type="button" className="icon-button" aria-label="Search" data-tip="Search (/)" onClick={openSearch}>
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

        <ul className="outline">
          {rows.map((row, index) => {
            const item = outline[row.key];
            const name = item.title || "empty item";
            const icon = shownIcon(item.icon, images);
            const hasChildren = item.children.length > 0;
            const { through, stem } = lines[index];
            const toggle = () => apply(setCollapsed(outline, row.key, !item.collapsed));
            return (
              <li
                key={row.key}
                className={["row", hasChildren && "parent", item.done ? "done" : row.under && "under", icon !== null && "with-icon", thread === row.key && "threaded", at >= 0 && current === row.key && "current-match"]
                  .filter(Boolean)
                  .join(" ")}
              >
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
                    {icon !== null ? (
                      <>
                        <span className="glyph">
                          <TodoIconGlyph icon={icon} />
                        </span>
                        {item.done && <DoneBadge />}
                      </>
                    ) : item.done ? (
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
                      onFocus={() => followCaret(row.key)}
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
                </span>
                <button
                  type="button"
                  className="set-icon"
                  aria-label={`${item.icon === null ? "Set icon for" : "Change icon of"} ${name}`}
                  data-tip="Icon (Ctrl/⌘ ⇧ E)"
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => openIcon(row.key)}
                >
                  <Icon name="emoji" size={18} />
                </button>
                <button
                  type="button"
                  className={["comment-button", item.comments > 0 && "has-comments", thread === row.key && "open"].filter(Boolean).join(" ")}
                  aria-label={`${item.comments === 0 ? "Comment on" : `${item.comments} ${item.comments === 1 ? "comment" : "comments"} on`} ${name}`}
                  aria-expanded={thread === row.key}
                  data-tip="Comments (⇧ Enter)"
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => openThread(row.key)}
                >
                  <Icon name="comment" size={18} />
                  {item.comments > 0 && <span>{item.comments}</span>}
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

        <button type="button" className="add-item" aria-label="Add item" data-tip="Add item" onClick={() => apply(append(outline, ROOT, fresh()))}>
          <Icon name="add" size={26} />
        </button>
      </main>

      {thread !== null && threadItem && (
        <ThreadPane
          key={thread}
          todoKey={thread}
          title={threadItem.title}
          path={pathOf(thread)}
          done={threadItem.done}
          count={threadItem.comments}
          serverId={sync.idOf(thread)}
          query={query}
          sync={sync}
          editable={!failed}
          avatar={<Avatar url={avatarUrl} size={28} />}
          focusRequest={threadFocus?.key === thread ? threadFocus.request : 0}
          draft={drafts.get(thread) ?? ""}
          onDraft={editDraft}
          onCount={countComments}
          onBack={() => focusRow({ key: thread, pos: -1 })}
          onClose={() => {
            setThread(null);
            focusRow({ key: thread, pos: -1 });
          }}
        />
      )}
      {moving && outline[moving.key] && (
        <MovePicker outline={outline} moving={moving.key} showDone={showDone} onMove={moveRow} onClose={closeMove} />
      )}
      {picking && outline[picking.key] && (
        <IconPicker
          outline={outline}
          picking={picking.key}
          images={images}
          onPick={pickIcon}
          onAdded={addedImage}
          onRenamed={renamedImage}
          onDeleted={deletedImage}
          onClose={closeIcon}
        />
      )}
      {confirming && outline[confirming.key] && (
        <ConfirmDialog outline={outline} confirming={confirming} onConfirm={confirm} onClose={closeConfirm} />
      )}
    </>
  );
}
