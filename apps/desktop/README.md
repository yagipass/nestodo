# Nestodo for the desktop

Manage your todos in a native macOS window, with the same outline editing and keyboard shortcuts as the web app.

## Install

Build the app with Nix, from the repo root:

```sh
cd apps/desktop
nix develop ../..#cli --command ./bundle.sh
```

Then move `target/release/Nestodo.app` to Applications.

## Usage

- Enter the address of your Nestodo server on first launch. Change it later from the account menu.
- Sign in with the same Cloudflare Access account as the web app. The login is kept in the macOS keychain. When the keychain can't be used, it goes to a file only you can read instead and stays there until you sign in again or sign out. The app renews the login without the browser until it fully expires.
- **Help** in the account menu lists the keyboard shortcuts. **Settings** opens the web app's settings in your browser.
- Light and dark follow the system setting.

## Development

See [CONTRIBUTING.md](../../CONTRIBUTING.md).
