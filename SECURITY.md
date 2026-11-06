# Security

## Reporting a vulnerability

Report vulnerabilities privately through [GitHub's private vulnerability reporting](https://github.com/yagipass/nestodo/security/advisories/new). Do not open a public issue.

## Supported versions

Only the latest release receives security fixes.

## Scope

Nestodo is a todo list for one family. Every request to the web app, the API and the MCP endpoint goes through Cloudflare Access, and each account sees only its own todos. Report anything that breaks this, such as reading or changing another account's todos, reaching them without signing in, or taking the login that the CLI or the Android app keeps. Do not test against the deployment my family uses. Report flaws in Cloudflare Access to Cloudflare.
