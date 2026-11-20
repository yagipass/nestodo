# Nestodo for the web

Manage your todos in the browser.

## Webhook

In **Settings**, enter a URL, an optional bearer token or other headers, and the title of a top-level todo. Within 13 minutes of adding a todo under it with no comments or only a link, the server receives its id, title and comments:

```json
{
  "event": "todos.awaiting_note",
  "project": "Reading list",
  "todos": [{ "id": 12, "title": "Read the changelog", "comments": ["https://example.com"] }]
}
```

To answer, add a comment to the todo with the MCP tool `add_comment`.

A failed send is not retried. The reason appears in **Settings**.

The URL and header values are marked **Secret** by default. A secret value cannot be shown again after saving, so turn it off for values you want to read later.

## Usage analytics

If the server has PostHog set up, **Settings** has a **Send usage data** switch. It is off until you turn it on and applies to your account on every device. When it is on, the pages and screens you open in the web, desktop and Android apps, your browser or device, and your approximate location from your IP address are sent to PostHog. Your todos and email address are never sent.

## Development

See [CONTRIBUTING.md](../../CONTRIBUTING.md).
