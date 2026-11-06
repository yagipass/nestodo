const REACT_FORM_REPLAY_SCRIPT = "'sha256-CwHpiObGc0Kj6qtRFzUVGqk0apwmCrylJaQU+RajuJ0='";

export const CONTENT_SECURITY_POLICY = [
  "default-src 'self'",
  `script-src 'self' ${REACT_FORM_REPLAY_SCRIPT}`,
  "style-src 'self' 'unsafe-inline'",
  "connect-src 'self' https://us.i.posthog.com https://us-assets.i.posthog.com",
  "object-src 'none'",
  "base-uri 'none'",
  "form-action 'self'",
  "frame-ancestors 'none'",
].join("; ");
