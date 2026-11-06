import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import type { BeforeSendFn } from "posthog-js";
import { analyticsAvailable, applyAnalyticsConsent, startAnalytics } from "./analytics";

type Config = {
  opt_out_capturing_by_default?: boolean;
  opt_out_persistence_by_default?: boolean;
  disable_external_dependency_loading?: boolean;
  before_send?: BeforeSendFn;
};

const posthog = vi.hoisted(() => {
  const browser = { consent: undefined as "granted" | "denied" | undefined };
  return {
    browser,
    __loaded: false,
    config: {} as Config,
    capturing: false,
    keepsId: false,
    init(_token: string, config: Config) {
      this.__loaded = true;
      this.config = config;
      this.capturing = browser.consent === "granted" || (browser.consent === undefined && !config.opt_out_capturing_by_default);
      this.keepsId = this.capturing || !config.opt_out_persistence_by_default;
    },
    opt_in_capturing() {
      browser.consent = "granted";
      this.capturing = true;
      this.keepsId = true;
    },
    opt_out_capturing() {
      browser.consent = "denied";
      this.capturing = false;
      this.keepsId = !this.config.opt_out_persistence_by_default;
    },
    reload() {
      this.__loaded = false;
      this.capturing = false;
    },
    clear() {
      browser.consent = undefined;
      this.reload();
      this.keepsId = false;
      this.config = {};
    },
  };
});

vi.mock("posthog-js", () => ({ default: posthog }));

function accountConsent(enabled: boolean | null) {
  const fetch = vi.fn(async () => (enabled === null ? new Response(null, { status: 500 }) : Response.json({ enabled })));
  vi.stubGlobal("fetch", fetch);
  return fetch;
}

beforeEach(() => {
  posthog.clear();
  vi.stubEnv("VITE_POSTHOG_PROJECT_TOKEN", "phc_test");
  vi.stubEnv("DEV", false);
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

describe("usage analytics", () => {
  it("never loads PostHog for a member who has not agreed, so nothing about them reaches PostHog", async () => {
    accountConsent(false);

    await startAnalytics();

    expect(posthog.__loaded).toBe(false);
  });

  it("sends page views once the member's account has agreed", async () => {
    accountConsent(true);

    await startAnalytics();

    expect(posthog.capturing).toBe(true);
  });

  it("keeps PostHog from adding its own scripts to the page, which the Content Security Policy blocks with an error in the member's console", async () => {
    accountConsent(true);

    await startAnalytics();

    expect(posthog.config.disable_external_dependency_loading).toBe(true);
  });

  it("turns off every kind of capture besides page views, so a switch flipped in the PostHog project or a newer SDK default cannot start sending what members click or the errors they run into", async () => {
    accountConsent(true);

    await startAnalytics();

    expect(posthog.config).toMatchObject({
      autocapture: false,
      rageclick: false,
      capture_dead_clicks: false,
      capture_heatmaps: false,
      capture_exceptions: false,
      disable_session_recording: true,
    });
  });

  it("drops query strings, fragments and referrers before anything is sent, so whatever a page puts in its URL or the page a member came from never reaches PostHog", async () => {
    accountConsent(true);
    await startAnalytics();

    const pageview = {
      uuid: "1",
      event: "$pageview",
      properties: {
        $current_url: "https://nestodo.example/stats?tz=Asia%2FTokyo#top",
        $pathname: "/stats",
        $referrer: "https://mail.example/inbox?id=1",
        $referring_domain: "mail.example",
      },
      $set_once: { $initial_current_url: "https://nestodo.example/?token=secret", $initial_referrer: "https://mail.example/" },
    };

    expect(posthog.config.before_send!(pageview)).toEqual({
      uuid: "1",
      event: "$pageview",
      properties: { $current_url: "https://nestodo.example/stats", $pathname: "/stats" },
      $set_once: { $initial_current_url: "https://nestodo.example/" },
    });
  });

  it("starts sending as soon as the member turns it on in the settings, without a reload", async () => {
    accountConsent(false);
    await startAnalytics();

    applyAnalyticsConsent(true);

    expect(posthog.capturing).toBe(true);
  });

  it("stops sending and forgets the anonymous id when the member turns it off, so later visits cannot be linked to earlier ones", async () => {
    accountConsent(true);
    await startAnalytics();

    applyAnalyticsConsent(false);

    expect(posthog.capturing).toBe(false);
    expect(posthog.keepsId).toBe(false);
  });

  it("follows the account in a browser where the member had turned it off, because the choice belongs to the account and not the device", async () => {
    accountConsent(true);
    await startAnalytics();
    applyAnalyticsConsent(false);
    posthog.reload();

    await startAnalytics();

    expect(posthog.capturing).toBe(true);
  });

  it("keeps PostHog unloaded when the consent check fails, because sending without a confirmed yes is not allowed", async () => {
    accountConsent(null);

    await startAnalytics();

    expect(posthog.__loaded).toBe(false);
  });

  it("sends nothing and hides the setting when the deployment has no PostHog token, so a copy that never set PostHog up stays silent", async () => {
    vi.stubEnv("VITE_POSTHOG_PROJECT_TOKEN", "");
    const fetch = accountConsent(true);

    await startAnalytics();
    applyAnalyticsConsent(true);

    expect(fetch).not.toHaveBeenCalled();
    expect(posthog.__loaded).toBe(false);
    expect(analyticsAvailable()).toBe(false);
  });

  it("sends nothing from the dev server even for a member who agreed, so testing stays out of the usage numbers", async () => {
    vi.stubEnv("DEV", true);
    accountConsent(true);

    await startAnalytics();
    applyAnalyticsConsent(true);

    expect(posthog.__loaded).toBe(false);
    expect(analyticsAvailable()).toBe(true);
  });
});
