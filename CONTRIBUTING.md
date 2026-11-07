# Contributing

I develop Nestodo for my family and do not take issues or pull requests from others. To report a vulnerability, see [SECURITY.md](SECURITY.md).

## Setup

Install [Nix](https://nixos.org/) with flakes and [direnv](https://direnv.net/), then run `direnv allow` at the repo root, in `apps/cli` and in `apps/android`. Each directory loads its dev shell from `flake.nix` and installs the git hooks. Without direnv, use `nix develop`, `nix develop .#cli` or `nix develop .#android`.

- The root shell provides Node.js and pnpm. Run `pnpm install` at the root.
- The `cli` shell provides the Rust toolchain.
- The `android` shell provides the Android SDK, the emulator (on Apple silicon) and JDK 25, and writes the SDK path to `apps/android/local.properties`. Without direnv, run Gradle from `apps/android` as `nix develop ../..#android -c ./gradlew ...`.

## Build and test

| Command | Purpose |
|---|---|
| `nix flake check` | run the formatters and git hooks over every tracked file, as CI does |
| `nix run --inputs-from . nixpkgs#gitleaks -- git --log-opts="--full-history --all --diff-filter=uxdb --diff-merges=first-parent" --redact --verbose` | scan every commit for secrets, as CI does |

### Web

Run these in `apps/web`.

| Command | Purpose |
|---|---|
| `pnpm exec vp dev` | serve the app at <http://localhost:5173>, signed in as `dev@localhost` |
| `pnpm exec void db generate` | generate a migration after changing `db/schema.ts`; commit the new migration |
| `pnpm exec void prepare` | generate `.void/`, which the type checks need |
| `pnpm exec tsc --noEmit && pnpm exec tsc --noEmit -p e2e` | type-check the app and the e2e tests |
| `pnpm exec vp build` | build the app |
| `pnpm exec vp test run` | run the unit and e2e tests |

CI runs `void prepare`, both type checks, `vp build` and `vp test run`.

### CLI

Run these in `apps/cli`.

| Command | Purpose |
|---|---|
| `cargo clippy --locked --all-targets -- -D warnings` | lint, as CI does |
| `cargo test --locked` | run the tests, as CI does |
| `NESTODO_E2E_BASE_URL=http://localhost:5173 cargo test -- --ignored` | run the end-to-end tests against `vp dev` |
| `cargo run -- --base-url http://localhost:5173 list` | run a command against `vp dev`, without signing in; `NESTODO_BASE_URL` sets the same |

CI also builds the Nix package with `nix build .#nestodo` on macOS (Apple silicon) and Linux (x86_64 and arm64), and pushes it to the `nestodo` Cachix cache from `main`.

### Android

Run these in `apps/android`. Gradle needs the address of your server: set `nestodo.baseUrl=https://nestodo.example.com` in `~/.gradle/gradle.properties`, or pass `-Pnestodo.baseUrl=https://nestodo.example.com`, which takes precedence.

| Command | Purpose |
|---|---|
| `./gradlew spotlessApply` | format Kotlin with ktfmt |
| `./gradlew spotlessCheck test assembleDebug` | check the formatting, run the unit tests and build, as CI does |
| `./gradlew installDebug` | build and install on every connected device |
| `./gradlew installRelease` | the same, minified with R8 so it scrolls smoothly |
| `./gradlew installDebug -Pnestodo.baseUrl=http://10.0.2.2:5173` | install a debug build that uses `vp dev` instead of production, at its address as seen from the emulator |

The release build is signed with the debug key, so it installs over a debug build and keeps the sign-in. Set `ANDROID_SERIAL` to install on one device only.

#### Emulator

```sh
avdmanager create avd -n nestodo -k "system-images;android-37.0;google_apis;arm64-v8a"
emulator -avd nestodo
```

Start the web app with `pnpm exec vp dev --host 127.0.0.1` for a debug build on the emulator: plain `vp dev` listens only on `::1`, which the emulator's `10.0.2.2` can't reach.

## Rules

- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/), checked by the `commit-msg` hook. Scope them to the app, such as `feat(web)`, `fix(android)` or `build(nix)`, and say what changes for the user: `feat(web): expand or collapse a todo by clicking its bullet`.
- Name branches `<type>/<slug>` after the Conventional Commits type, for example `feat/collapse-todos`. `.github/labeler.yml` labels the pull request by it, and the label picks its section of the release notes.
- tagpr writes `CHANGELOG.md` and the version numbers of the CLI, the Android app and the MCP server. Do not edit them by hand.

## Making a change

Branch from `main` and open a pull request. A pull request:

- has a Conventional Commits title. Pull requests are squash-merged, so the title becomes the commit on `main` and the line in `CHANGELOG.md`.
- passes the checks above for every app it changes.

## Release

Merge the release pull request that [tagpr](https://github.com/Songmu/tagpr) keeps open. Label it `tagpr:minor` or `tagpr:major` for more than a patch release. `release.yml` then tags `vX.Y.Z`, deploys the web app to Cloudflare and publishes the GitHub release.

The deploy runs in the `Cloudflare` environment, which only `main` can deploy from. It needs the environment's secrets `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`, `NESTODO_HOST` and `NESTODO_MCP_HOST`, and stops before deploying when `CLOUDFLARE_ACCOUNT_ID`, `NESTODO_HOST` or `NESTODO_MCP_HOST` is empty. The hosts are secrets so that the public workflow logs mask them.

The environment's secret `POSTHOG_PROJECT_TOKEN` is optional. Leave it empty to send nothing to PostHog and hide the usage analytics setting. When it is set, the web app sends page views only for users who turned on **Send usage data** in **Settings**. The dev server never sends.

`void.lock.json` is not committed. Each deploy finds the D1 database `nestodo-db` by name in `CLOUDFLARE_ACCOUNT_ID` and creates it only when it does not exist, so renaming the database in Cloudflare or in `void.config.ts` starts from an empty one. Void keeps Durable Object migration history in the lock, so commit it again before adding a Durable Object.

## Repository settings

The repository settings, labels, merge options, and Actions permissions are managed with [gh-infra](https://github.com/babarot/gh-infra) in `.github/infra.yml`. To change them, edit the file, review the difference with `gh infra plan .github/infra.yml`, and apply it with `gh infra apply .github/infra.yml`. Applying needs admin access to the repository.

gh-infra creates the `Cachix` and `Cloudflare` environments and limits them to `main`, but does not manage secrets or variables. Set them under **Settings → Environments** on GitHub. The `Cachix` environment needs the secret `CACHIX_AUTH_TOKEN`, a token that can push to the `nestodo` Cachix cache.
