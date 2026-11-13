# Nestodo CLI

Manage your todos from the terminal.

## Install

On macOS (Apple silicon) and Linux, with Nix:

```sh
nix profile install github:yagipass/nestodo#nestodo
```

## Usage

```sh
export NESTODO_BASE_URL=https://nestodo.example.com  # your server; --base-url also sets it
nestodo login                         # sign in with your browser (once)
nestodo list [ID] [-d N] [--active] [--json]
nestodo add "Buy milk" [-p ID | --after ID]
nestodo mv 7 8 --to 3                 # or --after ID to put them right after a todo
nestodo indent 7                      # nest under the todo above
nestodo outdent 7                     # move out to right after its parent
nestodo show 7
nestodo edit 7 --title "Buy oat milk"
nestodo done 7 8 [-r]                 # -r also marks the open todos under it as done
nestodo undo 7
nestodo rm 7 [-r]                     # -r also deletes the todos under it
nestodo logout
```

IDs are the numbers `nestodo list` shows.

`nestodo login` opens the same Cloudflare Access sign-in as the web app. The login is kept in your OS keychain. When the keychain can't be used, it goes to a file only you can read instead and stays there until you log in again or log out. Later commands renew the login without the browser until it fully expires.

## Development

See [CONTRIBUTING.md](../../CONTRIBUTING.md).
