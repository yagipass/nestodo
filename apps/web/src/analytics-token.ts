export function projectToken() {
  return import.meta.env.VITE_POSTHOG_PROJECT_TOKEN;
}

export function analyticsAvailable() {
  return Boolean(projectToken());
}
