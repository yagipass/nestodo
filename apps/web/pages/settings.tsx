import { useEffect, useRef, useState, type FormEvent } from "react";
import { action, Form, useForm, useRouter, useShared } from "@void/react";
import { analyticsAvailable, applyAnalyticsConsent } from "../src/analytics";
import { avatarImageFrom, base64Of } from "../src/avatar-image";
import { Icon } from "../src/icons";
import { MAX_NAME_LENGTH, type Profile } from "../src/profiles";
import { AppHeader, Avatar } from "./layout";
import type { Props } from "./settings.server";

type Webhook = NonNullable<Props["webhook"]>;

const HIDDEN = "••••••••";

function Hidden() {
  return (
    <span role="img" aria-label="Hidden">
      {HIDDEN}
    </span>
  );
}

function SecretToggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return (
    <label className="secret-toggle">
      <input type="checkbox" aria-label={label} checked={checked} onChange={(event) => onChange(event.target.checked)} />
      Secret
    </label>
  );
}

function WebhookForm({ webhook, onDone }: { webhook: Webhook | null; onDone: () => void }) {
  const saved = webhook?.unreadable ? null : webhook;
  const form = useForm("/settings?save", {
    url: webhook?.url ?? "",
    urlSecret: webhook ? webhook.url === null : true,
    token: "" as string | null,
    headers: webhook?.headers ?? [],
    project: webhook?.project ?? "",
  });
  const { data, setData, wasSuccessful } = form;
  const sameUrl = data.url === (saved?.url ?? "");
  const tokenKept = saved?.hasToken && sameUrl && data.token === "";

  useEffect(() => {
    if (wasSuccessful) onDone();
  }, [wasSuccessful, onDone]);

  const setHeader = (index: number, change: Partial<Webhook["headers"][number]>) => {
    setData(
      "headers",
      data.headers.map((header, at) => (at === index ? { ...header, ...change } : header)),
    );
  };

  const valueKept = ({ name, secret }: Webhook["headers"][number]) =>
    secret && sameUrl && saved?.headers.some((header) => header.secret && header.name.toLowerCase() === name.trim().toLowerCase());

  const remove = async () => {
    if (!window.confirm("Remove this webhook? Its URL, token and headers cannot be shown again.")) return;
    const result = await action("/settings?remove", { method: "DELETE" });
    if (result.ok) onDone();
  };

  return (
    <Form form={form} onReset={(event) => event.preventDefault()}>
      <div className="field">
        <label htmlFor="webhook-url">URL</label>
        <div className="field-row">
          <input
            id="webhook-url"
            name="url"
            type={data.urlSecret ? "password" : "url"}
            autoComplete={data.urlSecret ? "new-password" : "off"}
            placeholder={saved?.url === null && data.urlSecret ? "Leave blank to keep the saved URL" : "https://example.com/hook"}
            aria-invalid={form.errors.url ? true : undefined}
            value={data.url}
            onChange={(event) => setData("url", event.target.value)}
          />
          <SecretToggle label="Keep the URL secret" checked={data.urlSecret} onChange={(checked) => setData("urlSecret", checked)} />
        </div>
        {saved && <span className="field-hint">A new URL drops the saved token and asks for secret header values again, so they never reach another server.</span>}
      </div>
      <div className="field">
        <label htmlFor="webhook-token">Token</label>
        <div className="field-row">
          <input
            id="webhook-token"
            name="token"
            type="password"
            autoComplete="new-password"
            placeholder={tokenKept ? HIDDEN : "Optional"}
            aria-invalid={form.errors.token ? true : undefined}
            value={data.token ?? ""}
            onChange={(event) => setData("token", event.target.value)}
          />
          {tokenKept && (
            <button type="button" className="icon-button" aria-label="Remove the token" onClick={() => setData("token", null)}>
              <Icon name="close" size={18} />
            </button>
          )}
        </div>
        <span className="field-hint">Sent as a bearer token and never shown again.</span>
      </div>
      <div className="field" role="group" aria-labelledby="webhook-headers">
        <span id="webhook-headers">Headers</span>
        {data.headers.map((header, index) => (
          <div className="field-row" key={index}>
            <input
              aria-label={`Header ${index + 1} name`}
              placeholder="Name"
              autoComplete="off"
              autoCapitalize="none"
              spellCheck={false}
              value={header.name}
              onChange={(event) => setHeader(index, { name: event.target.value })}
            />
            <input
              aria-label={`Header ${index + 1} value`}
              type={header.secret ? "password" : "text"}
              placeholder={valueKept(header) ? HIDDEN : "Value"}
              autoComplete={header.secret ? "new-password" : "off"}
              autoCapitalize="none"
              spellCheck={false}
              value={header.value}
              onChange={(event) => setHeader(index, { value: event.target.value })}
            />
            <SecretToggle label={`Keep header ${index + 1} secret`} checked={header.secret} onChange={(secret) => setHeader(index, { secret })} />
            <button
              type="button"
              className="icon-button"
              aria-label={`Remove header ${index + 1}`}
              onClick={() =>
                setData(
                  "headers",
                  data.headers.filter((_, at) => at !== index),
                )
              }
            >
              <Icon name="close" size={18} />
            </button>
          </div>
        ))}
        <button type="button" className="text-button" onClick={() => setData("headers", [...data.headers, { name: "", value: "", secret: true }])}>
          Add header
        </button>
        <span className="field-hint">A secret value is never shown again.{saved && " Leave it blank to keep it."}</span>
      </div>
      <label className="field">
        Project
        <input
          name="project"
          required
          autoComplete="off"
          aria-invalid={form.errors.project ? true : undefined}
          value={data.project}
          onChange={(event) => setData("project", event.target.value)}
        />
        <span className="field-hint">The title of a top-level todo. Only todos under it are sent.</span>
      </label>
      <div className="settings-actions">
        <button type="submit" className="pill" disabled={form.pending}>
          Save
        </button>
        {webhook && (
          <>
            <button type="button" className="text-button" onClick={onDone}>
              Cancel
            </button>
            <button type="button" className="text-button settings-remove" onClick={remove}>
              Remove
            </button>
          </>
        )}
      </div>
    </Form>
  );
}

function WebhookView({ webhook }: { webhook: Webhook }) {
  return (
    <dl className="webhook-view">
      <dt>URL</dt>
      <dd>
        {webhook.url ?? <Hidden />}
      </dd>
      <dt>Token</dt>
      <dd>{webhook.hasToken ? <Hidden /> : "None"}</dd>
      <dt>Headers</dt>
      <dd>
        {webhook.headers.length === 0 ? (
          "None"
        ) : (
          <ul>
            {webhook.headers.map(({ name, value, secret }) => (
              <li key={name}>
                {name}: {secret ? <Hidden /> : value}
              </li>
            ))}
          </ul>
        )}
      </dd>
      <dt>Project</dt>
      <dd>{webhook.project}</dd>
    </dl>
  );
}

function WebhookStatus({ webhook }: { webhook: Webhook }) {
  return (
    <>
      {webhook.lastSentAt && <p className="webhook-status">Last sent {webhook.lastSentAt} UTC.</p>}
      {webhook.lastError && (
        <p className="webhook-error" role="alert">
          The last send failed: {webhook.lastError}.
        </p>
      )}
      {webhook.unreadable && (
        <p className="webhook-error" role="alert">
          The saved URL, token and headers can no longer be read. Edit this webhook to enter them again.
        </p>
      )}
    </>
  );
}

type ProfileErrors = { name?: string; image?: string; form?: string };

const JSON_HEADERS = { Accept: "application/json", "Content-Type": "application/json" };

async function profileErrorsOf(res: Response | null): Promise<ProfileErrors> {
  if (res?.status !== 400) return { form: "The profile could not be saved. Try again." };
  const body = (await res.json().catch(() => null)) as { issues?: { issues: { path: string; message: string }[] }[] } | null;
  const issues = body?.issues?.flatMap(({ issues }) => issues) ?? [];
  return Object.fromEntries(issues.map(({ path, message }) => [path === "name" || path === "image" ? path : "form", message]));
}

function ProfileCard({ profile }: { profile: Profile }) {
  const router = useRouter();
  const picker = useRef<HTMLInputElement>(null);
  const [name, setName] = useState(profile.name ?? "");
  const [busy, setBusy] = useState(false);
  const [errors, setErrors] = useState<ProfileErrors>({});

  const send = async (path: string, method: string, body?: unknown) => {
    setBusy(true);
    setErrors({});
    const res = await fetch(path, { method, headers: JSON_HEADERS, body: body === undefined ? undefined : JSON.stringify(body) }).catch(() => null);
    setBusy(false);
    if (!res?.ok) {
      setErrors(await profileErrorsOf(res));
      return null;
    }
    router.refresh();
    return res;
  };

  const saveName = async (event: FormEvent) => {
    event.preventDefault();
    const res = await send("/api/v1/profile", "PUT", { name });
    if (res) setName(((await res.json()) as Profile).name ?? "");
  };

  const chooseImage = async (file: File | undefined) => {
    if (picker.current) picker.current.value = "";
    if (!file) return;
    setBusy(true);
    setErrors({});
    const image = await avatarImageFrom(file)
      .then(base64Of)
      .catch(() => null);
    if (image === null) {
      setBusy(false);
      setErrors({ image: "The image could not be read." });
      return;
    }
    await send("/api/v1/profile/avatar", "PUT", { image });
  };

  return (
    <section className="card" aria-labelledby="profile-title">
      <h2 id="profile-title">Profile</h2>
      <p className="card-note">Shown in the account menu on every device.</p>
      {errors.form && (
        <p className="webhook-error" role="alert">
          {errors.form}
        </p>
      )}
      <div className="profile-picture">
        <Avatar url={profile.avatarUrl} size={64} />
        <div className="profile-picture-actions">
          <button type="button" className="pill" disabled={busy} onClick={() => picker.current?.click()}>
            Choose image
          </button>
          {profile.avatarUrl && (
            <button
              type="button"
              className="text-button"
              aria-label="Remove picture"
              disabled={busy}
              onClick={() => send("/api/v1/profile/avatar", "DELETE")}
            >
              Remove
            </button>
          )}
        </div>
        <input ref={picker} type="file" accept="image/*" hidden aria-label="Image" onChange={(event) => chooseImage(event.target.files?.[0])} />
      </div>
      {errors.image && (
        <p className="webhook-error" role="alert">
          {errors.image}
        </p>
      )}
      <form className="field" onSubmit={saveName}>
        <label htmlFor="profile-name">Name</label>
        <div className="field-row">
          <input
            id="profile-name"
            name="name"
            autoComplete="name"
            maxLength={MAX_NAME_LENGTH}
            placeholder={profile.email}
            aria-invalid={errors.name ? true : undefined}
            aria-describedby="profile-name-hint"
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
          <button type="submit" className="pill" disabled={busy}>
            Save
          </button>
        </div>
        {errors.name && (
          <span className="webhook-error" role="alert">
            {errors.name}
          </span>
        )}
        <span id="profile-name-hint" className="field-hint">
          Shown instead of your email address. Leave it blank to show the email address.
        </span>
      </form>
    </section>
  );
}

function AnalyticsCard({ consented }: { consented: boolean }) {
  const [enabled, setEnabled] = useState(consented);
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState(false);

  const change = async (next: boolean) => {
    setSaving(true);
    setFailed(false);
    const res = await fetch("/api/v1/analytics", {
      method: "PUT",
      headers: { Accept: "application/json", "Content-Type": "application/json" },
      body: JSON.stringify({ enabled: next }),
    }).catch(() => null);
    setSaving(false);
    if (!res?.ok) {
      setFailed(true);
      return;
    }
    setEnabled(next);
    applyAnalyticsConsent(next);
  };

  return (
    <section className="card" aria-labelledby="analytics-title">
      <h2 id="analytics-title">Usage analytics</h2>
      <p className="card-note">
        When on, the pages and screens you open in the web, desktop and Android apps, your browser or device, and your approximate location from your IP address are sent
        to PostHog. Your todos and email address are never sent. This applies to your account on every device.
      </p>
      {failed && (
        <p className="webhook-error" role="alert">
          The setting could not be saved. Try again.
        </p>
      )}
      <label className="analytics-toggle">
        <input type="checkbox" checked={enabled} disabled={saving} onChange={(event) => change(event.target.checked)} />
        Send usage data
      </label>
    </section>
  );
}

export default function SettingsPage({ available, webhook, analytics }: Props) {
  const profile = useShared()?.auth.profile ?? null;
  const [editing, setEditing] = useState(false);
  const stopEditing = () => setEditing(false);
  const viewing = available && webhook !== null && !editing;

  return (
    <>
      <AppHeader />
      <main className="settings">
        <h1>Settings</h1>
        {profile && <ProfileCard profile={profile} />}
        <section className="card" aria-labelledby="webhook-title">
          <div className="card-head">
            <h2 id="webhook-title">Webhook</h2>
            {viewing && (
              <button type="button" className="pill" onClick={() => setEditing(true)}>
                Edit
              </button>
            )}
          </div>
          <p className="card-note">
            Within 13 minutes of adding a todo with no note or only a link, its id, title and note are sent to this URL.
          </p>
          {available ? (
            <>
              {webhook && <WebhookStatus webhook={webhook} />}
              {viewing ? <WebhookView webhook={webhook} /> : <WebhookForm key={webhook ? "edit" : "new"} webhook={webhook} onDone={stopEditing} />}
            </>
          ) : (
            <p className="webhook-error">Webhooks are not available because WEBHOOK_SECRET_KEY is not set on this server.</p>
          )}
        </section>
        {analyticsAvailable() && <AnalyticsCard consented={analytics} />}
      </main>
    </>
  );
}
