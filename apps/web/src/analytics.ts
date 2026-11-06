import posthog, { type CaptureResult, type Properties } from "posthog-js";

function withoutQuery(value: unknown) {
  if (typeof value !== "string" || !/^https?:\/\//i.test(value) || !URL.canParse(value)) return value;
  const { origin, pathname } = new URL(value);
  return origin + pathname;
}

function scrubbed(properties: Properties) {
  return Object.fromEntries(
    Object.entries(properties)
      .filter(([key]) => !key.includes("referr"))
      .map(([key, value]) => [key, withoutQuery(value)]),
  );
}

function withoutQueriesOrReferrers(event: CaptureResult): CaptureResult {
  return {
    ...event,
    properties: scrubbed(event.properties),
    ...(event.$set && { $set: scrubbed(event.$set) }),
    ...(event.$set_once && { $set_once: scrubbed(event.$set_once) }),
  };
}

function projectToken() {
  return import.meta.env.VITE_POSTHOG_PROJECT_TOKEN;
}

function sendingToken() {
  return import.meta.env.DEV ? undefined : projectToken();
}

export function analyticsAvailable() {
  return Boolean(projectToken());
}

export function applyAnalyticsConsent(enabled: boolean) {
  const token = sendingToken();
  if (!token) return;
  if (enabled) {
    if (!posthog.__loaded) {
      posthog.init(token, {
        api_host: "https://us.i.posthog.com",
        defaults: "2026-08-30",
        autocapture: false,
        rageclick: false,
        capture_dead_clicks: false,
        capture_heatmaps: false,
        capture_exceptions: false,
        disable_session_recording: true,
        disable_external_dependency_loading: true,
        before_send: withoutQueriesOrReferrers,
        opt_out_capturing_by_default: true,
        opt_out_persistence_by_default: true,
      });
    }
    posthog.opt_in_capturing({ captureEventName: false });
  } else if (posthog.__loaded) {
    posthog.opt_out_capturing();
  }
}

export async function startAnalytics() {
  if (!sendingToken() || posthog.__loaded) return;
  const res = await fetch("/api/v1/analytics", { headers: { Accept: "application/json" } });
  if (!res.ok) return;
  const { enabled } = (await res.json()) as { enabled: boolean };
  applyAnalyticsConsent(enabled);
}
