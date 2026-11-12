import { defineConfig } from 'void/config';

const routes = [process.env.NESTODO_HOST, process.env.NESTODO_MCP_HOST]
  .filter((pattern): pattern is string => Boolean(pattern))
  .map((pattern) => ({ "pattern": pattern, "custom_domain": true }));

export default defineConfig({
  "worker": {
    "compatibility_date": "2026-02-24",
    "compatibility_flags": ["global_fetch_strictly_public"]
  },
  "cloudflare": {
    "name": "nestodo",
    "d1_databases": [
      { "binding": "DB", "database_name": "nestodo-db", "database_id": "local", "migrations_dir": "./db/migrations" }
    ],
    "r2_buckets": [
      { "binding": "STORAGE", "bucket_name": "nestodo-avatars" }
    ],
    ...(routes.length > 0 ? { "routes": routes } : {}),
    "workers_dev": false,
    "preview_urls": false,
    "observability": {
      "logs": {
        "enabled": true,
        "invocation_logs": true,
        "persist": true
      }
    }
  },
  "head": {
    "titleTemplate": "%s | Nestodo",
    "htmlAttrs": { "lang": "en" },
    "meta": [
      { "name": "viewport", "content": "width=device-width, initial-scale=1" }
    ],
    "link": [
      { "rel": "manifest", "href": "/manifest.webmanifest", "crossorigin": "use-credentials" },
      { "rel": "icon", "href": "/icon.svg", "type": "image/svg+xml" },
      { "rel": "apple-touch-icon", "href": "/apple-touch-icon.png" }
    ]
  },
  "routing": {
    "headers": {
      "/*": [
        "X-Content-Type-Options: nosniff",
        "X-Frame-Options: DENY",
        "Referrer-Policy: strict-origin-when-cross-origin",
        "Strict-Transport-Security: max-age=31536000",
        "X-Robots-Tag: noindex"
      ]
    }
  }
});
