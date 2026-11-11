# Changelog

## [v0.10.0](https://github.com/yagipass/nestodo/compare/v0.9.3...v0.10.0) - 2026-11-11

### Web
- build(web): bump the minor-and-patch group with 12 updates by @dependabot[bot] in https://github.com/yagipass/nestodo/pull/32
- build(web): bump the minor-and-patch group with 3 updates by @dependabot[bot] in https://github.com/yagipass/nestodo/pull/53
### Android
- feat(android): ask for the server address on first launch by @yagipass in https://github.com/yagipass/nestodo/pull/36
- fix(android): keep the server and its sign-in until another is entered by @yagipass in https://github.com/yagipass/nestodo/pull/47
- feat(android): show the Nestodo icon next to the name in the top bar by @yagipass in https://github.com/yagipass/nestodo/pull/49
### Desktop
- feat(desktop): ask for the server address on first launch by @yagipass in https://github.com/yagipass/nestodo/pull/38
- fix(desktop): keep the server and its sign-in until another is entered by @yagipass in https://github.com/yagipass/nestodo/pull/46
- feat(desktop): put the header in the macOS title bar by @yagipass in https://github.com/yagipass/nestodo/pull/55
### New Features
- feat(desktop): add the sign-in, sync and outline core of a macOS app by @yagipass in https://github.com/yagipass/nestodo/pull/22
- feat(desktop): manage todos in a native macOS app by @yagipass in https://github.com/yagipass/nestodo/pull/23
- feat(desktop): install the desktop app with Nix by @yagipass in https://github.com/yagipass/nestodo/pull/42
### Dependencies
- build(cli): bump the minor-and-patch group with 2 updates by @dependabot[bot] in https://github.com/yagipass/nestodo/pull/28
- build(nix): bump the flake-inputs group with 2 updates by @dependabot[bot] in https://github.com/yagipass/nestodo/pull/30
- ci(actions): bump the minor-and-patch group across 1 directory with 2 updates by @dependabot[bot] in https://github.com/yagipass/nestodo/pull/29
- build(android): bump the minor-and-patch group in /apps/android with 3 updates by @dependabot[bot] in https://github.com/yagipass/nestodo/pull/31
- build(nix): bump the flake-inputs group with 2 updates by @dependabot[bot] in https://github.com/yagipass/nestodo/pull/52
### Maintenance
- build: keep npm, Cargo, Gradle and Actions dependencies up to date by @yagipass in https://github.com/yagipass/nestodo/pull/27
- ci: label desktop pull requests and group them in the release notes by @yagipass in https://github.com/yagipass/nestodo/pull/34
- ci(desktop): lint and test the desktop app on macOS by @yagipass in https://github.com/yagipass/nestodo/pull/41
- build(desktop): track current GPUI through gpui-pre snapshots by @yagipass in https://github.com/yagipass/nestodo/pull/51
- build(web): give vite-plus bumps their own Dependabot PR by @yagipass in https://github.com/yagipass/nestodo/pull/57

## [v0.9.3](https://github.com/yagipass/nestodo/compare/v0.9.2...v0.9.3) - 2026-11-09

### Web
- feat(web): show the Nestodo icon next to the name in the header by @yagipass in https://github.com/yagipass/nestodo/pull/8
### Android
- feat(android): edit a shared todo's title, with its link in the note by @yagipass in https://github.com/yagipass/nestodo/pull/12
- fix(android): let the cursor handle reach a title's hidden start by @yagipass in https://github.com/yagipass/nestodo/pull/14
- fix(android): let the cursor handle reach a title's hidden end by @yagipass in https://github.com/yagipass/nestodo/pull/15
### New Features
- feat(cli): install the CLI with Nix from the nestodo Cachix cache by @yagipass in https://github.com/yagipass/nestodo/pull/6
### Dependencies
- build(deps): bump nixpkgs from `f05535a` to `885ba4d` in the flake-inputs group by @dependabot[bot] in https://github.com/yagipass/nestodo/pull/2
### Maintenance
- chore: configure cubic and CodeRabbit reviews for this repository by @yagipass in https://github.com/yagipass/nestodo/pull/10
- refactor(android): use the KTX extensions that lint's UseKtx asks for by @yagipass in https://github.com/yagipass/nestodo/pull/18

## [v0.9.2](https://github.com/yagipass/nestodo/compare/v0.9.1...v0.9.2) - 2026-11-07

### Maintenance
- ci: read the PostHog token from an environment secret by @yagipass in https://github.com/yagipass/nestodo/pull/3
