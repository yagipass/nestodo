mod access;
mod api;
mod error;
mod http;
mod outline;
mod store;
mod todo;

use std::io::IsTerminal;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{ArgGroup, Args, Parser, Subcommand};
use url::Url;

use access::Session;
use api::Api;
use error::Error;
use http::{Http, is_loopback};
use outline::{Move, Outline, Removal};
use store::{Kind, SystemStore, TokenStore};
use todo::{Style, Todo, View};

#[derive(Parser)]
#[command(
    name = "nestodo",
    version,
    about = "Manage your todos from the terminal"
)]
struct Cli {
    /// Server to connect to
    #[arg(long, global = true, env = "NESTODO_BASE_URL", value_parser = parse_base_url)]
    base_url: Option<Url>,

    /// Print each HTTP request to stderr
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Log in with your browser and save the login
    Login,
    /// Forget the saved login
    Logout,
    /// Show todos as a tree
    List {
        /// Show only what is under this todo
        #[arg(value_name = "ID")]
        id: Option<Id>,
        /// How many levels to show
        #[arg(short, long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
        depth: Option<u32>,
        /// Hide done todos and everything under them
        #[arg(long)]
        active: bool,
        /// Print the tree as JSON
        #[arg(long)]
        json: bool,
    },
    /// Add a todo
    Add {
        title: String,
        /// Add it as the last todo under this one
        #[arg(short, long, value_name = "ID", conflicts_with = "after")]
        parent: Option<Id>,
        /// Add it right after this todo
        #[arg(long, value_name = "ID")]
        after: Option<Id>,
    },
    /// Move todos
    Mv(MvArgs),
    /// Nest a todo under the todo above it
    Indent { id: Id },
    /// Move a todo out of its parent
    Outdent { id: Id },
    /// Show a todo
    Show { id: Id },
    /// Change the title of a todo
    Edit {
        id: Id,
        /// New title
        #[arg(long)]
        title: String,
    },
    /// Mark todos as done
    Done {
        #[arg(required = true, value_name = "ID")]
        ids: Vec<Id>,
        /// Also mark the open todos under them as done
        #[arg(short, long)]
        recursive: bool,
    },
    /// Mark todos as not done
    Undo {
        #[arg(required = true, value_name = "ID")]
        ids: Vec<Id>,
    },
    /// Delete todos
    Rm {
        #[arg(required = true, value_name = "ID")]
        ids: Vec<Id>,
        /// Also delete the todos under them
        #[arg(short, long)]
        recursive: bool,
    },
}

#[derive(Args)]
#[command(group(ArgGroup::new("target").required(true).multiple(true).args(["to", "after"])))]
struct MvArgs {
    #[arg(required = true, value_name = "ID")]
    ids: Vec<Id>,
    /// Move them to the end of this todo's children
    #[arg(long, value_name = "ID")]
    to: Option<Id>,
    /// Move them right after this todo
    #[arg(long, value_name = "ID")]
    after: Option<Id>,
}

type Id = u64;

fn parse_base_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| format!("{value:?} is not a URL: {error}"))?;
    let origin_only = url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none();
    if !origin_only || url.host_str().is_none() {
        return Err(format!(
            "{value:?} must be an origin such as https://nestodo.example.com"
        ));
    }
    match (url.scheme(), url.host_str()) {
        ("https", _) => Ok(url),
        ("http", Some(host)) if is_loopback(host) => Ok(url),
        ("http", _) => Err(format!(
            "{value:?} uses http://, which is only allowed for localhost"
        )),
        _ => Err(format!("{value:?} must start with https://")),
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {}", todo::printable(&error.to_string()));
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Error> {
    let base = cli.base_url.as_ref().ok_or_else(|| {
        Error::Invalid("no server to connect to; set NESTODO_BASE_URL or pass --base-url".into())
    })?;
    let http = Http::new(cli.verbose);
    let store = SystemStore::new();
    let host = base.host_str().unwrap_or_default();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64);

    match cli.command {
        Command::Login => {
            let app = access::discover(&http, base, now)?
                .ok_or_else(|| Error::NotProtected { host: host.into() })?;
            match access::login(&http, base, &app, &store, now)?.as_str() {
                "" => println!("Logged in to {host}."),
                email => println!("Logged in to {host} as {}.", todo::printable(email)),
            }
        }
        Command::Logout => {
            logout(&store, host)?;
            println!("Logged out of {host}.");
        }
        command => {
            if let Command::Add { title, .. } | Command::Edit { title, .. } = &command
                && title.trim().is_empty()
            {
                return Err(Error::Invalid("Enter a title".into()));
            }
            let session = match access::discover(&http, base, now)? {
                Some(app) => Some(Session::open(&http, base, app, &store, now)?),
                None => None,
            };
            let style = Style {
                color: std::io::stdout().is_terminal()
                    && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty()),
            };
            todos(command, &mut Api::new(&http, base, session, now), style)?;
        }
    }
    Ok(())
}

fn logout(store: &dyn TokenStore, host: &str) -> Result<(), Error> {
    let app = store.delete(host, Kind::App);
    let org = store.delete(host, Kind::Org);
    app.and(org)
}

fn todos(command: Command, api: &mut Api, style: Style) -> Result<(), Error> {
    match command {
        Command::List {
            id,
            depth,
            active,
            json,
        } => {
            let outline = Outline::new(api.list()?);
            let start = id.map(|id| outline.get(id)).transpose()?;
            let view = View { depth, active };
            if json {
                println!("{}", todo::json(&outline, start, &view));
            } else {
                println!("{}", todo::tree(&outline, start, &view, style));
            }
        }
        Command::Show { id } => {
            let outline = Outline::new(api.list()?);
            println!("{}", todo::detail(&outline, outline.get(id)?));
        }
        Command::Add {
            title,
            parent,
            after,
        } => {
            let outline = match (parent, after) {
                (None, None) => Outline::default(),
                _ => Outline::new(api.list()?),
            };
            let (parent, index) = match after {
                Some(after) => {
                    let anchor = outline.get(after)?;
                    (anchor.parent_id, Some(anchor.position + 1))
                }
                None => {
                    if let Some(parent) = parent {
                        outline.get(parent)?;
                    }
                    (parent, None)
                }
            };
            placed(&outline, &api.create(title.trim(), parent, index)?, style);
        }
        Command::Mv(args) => {
            let mut outline = Outline::new(api.list()?);
            for step in outline.plan_moves(&args.ids, args.to, args.after)? {
                api.move_to(step.id, step.parent, step.index)?;
                println!("Moved {} → {}", step.id, outline.path(step.parent));
            }
        }
        Command::Indent { id } => shift(api, id, Outline::indent, style)?,
        Command::Outdent { id } => shift(api, id, Outline::outdent, style)?,
        Command::Edit { id, title } => {
            change(api, &[id], style, |api, id| api.edit(id, title.trim()))?
        }
        Command::Done { ids, recursive } => complete(api, &ids, recursive, style)?,
        Command::Undo { ids } => change(api, &ids, style, |api, id| api.set_completed(id, false))?,
        Command::Rm { ids, recursive } => {
            let mut outline = Outline::new(api.list()?);
            for Removal { id, under } in outline.plan_removals(&ids, recursive)? {
                api.delete(id, under > 0)?;
                match under {
                    0 => println!("Deleted todo {id}."),
                    under => println!("Deleted todo {id} and {under} under it."),
                }
            }
        }
        Command::Login | Command::Logout => unreachable!("handled before connecting"),
    }
    Ok(())
}

fn change(
    api: &mut Api,
    ids: &[Id],
    style: Style,
    mut apply: impl FnMut(&mut Api, Id) -> Result<Todo, Error>,
) -> Result<(), Error> {
    let mut outline = Outline::new(api.list()?);
    for &id in ids {
        outline.get(id)?;
    }
    for &id in ids {
        outline.update(apply(api, id)?);
        println!("{}", todo::row(&outline, outline.get(id)?, style));
    }
    Ok(())
}

fn complete(api: &mut Api, ids: &[Id], recursive: bool, style: Style) -> Result<(), Error> {
    let mut outline = Outline::new(api.list()?);
    outline.check_completions(ids, recursive)?;
    for &id in ids {
        outline.update(api.set_completed(id, true)?);
        outline.complete_under(id);
        println!("{}", todo::row(&outline, outline.get(id)?, style));
    }
    Ok(())
}

fn shift(
    api: &mut Api,
    id: Id,
    plan: fn(&mut Outline, Id) -> Result<Move, Error>,
    style: Style,
) -> Result<(), Error> {
    let mut outline = Outline::new(api.list()?);
    let step = plan(&mut outline, id)?;
    placed(
        &outline,
        &api.move_to(step.id, step.parent, step.index)?,
        style,
    );
    Ok(())
}

fn placed(outline: &Outline, todo: &Todo, style: Style) {
    println!("{}", todo::row(outline, todo, style));
    println!(
        "{}",
        style.dim(&format!("  under {}", outline.path(todo.parent_id)))
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemoryStore;

    #[test]
    fn accepts_https_and_local_http_origins() {
        for ok in [
            "https://nestodo.example.com",
            "https://nestodo.example.com/",
            "http://localhost:5173",
            "http://127.0.0.1:8787",
            "http://[::1]:3000",
        ] {
            assert!(parse_base_url(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn refuses_plain_http_to_anything_but_localhost_so_tokens_never_travel_unencrypted() {
        for bad in [
            "http://nestodo.example.com",
            "http://192.168.1.10:5173",
            "http://localhost.evil.example",
        ] {
            assert!(parse_base_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn refuses_urls_that_are_not_plain_origins() {
        for bad in [
            "ftp://nestodo.example.com",
            "https://nestodo.example.com/api",
            "https://user@nestodo.example.com",
            "https://nestodo.example.com/?x=1",
            "nestodo.example.com",
        ] {
            assert!(parse_base_url(bad).is_err(), "{bad}");
        }
    }

    struct StuckToken(Kind, MemoryStore);

    impl TokenStore for StuckToken {
        fn load(&self, host: &str, kind: Kind) -> Result<Option<String>, Error> {
            self.1.load(host, kind)
        }

        fn save(&self, host: &str, kind: Kind, token: &str) -> Result<(), Error> {
            self.1.save(host, kind, token)
        }

        fn delete(&self, host: &str, kind: Kind) -> Result<(), Error> {
            if kind == self.0 {
                Err(Error::Storage("the keychain is locked".into()))
            } else {
                self.1.delete(host, kind)
            }
        }
    }

    #[test]
    fn logout_still_forgets_the_long_lived_org_token_when_the_app_token_cannot_be_deleted() {
        let store = StuckToken(Kind::App, MemoryStore::default());
        store.save("nestodo.example.com", Kind::App, "app").unwrap();
        store.save("nestodo.example.com", Kind::Org, "org").unwrap();

        let error = logout(&store, "nestodo.example.com").unwrap_err();

        assert_eq!(
            error.to_string(),
            "could not access the saved login: the keychain is locked"
        );
        assert_eq!(store.load("nestodo.example.com", Kind::Org).unwrap(), None);
    }

    #[test]
    fn logout_fails_while_the_long_lived_org_token_could_still_sign_the_next_command_back_in() {
        let store = StuckToken(Kind::Org, MemoryStore::default());
        store.save("nestodo.example.com", Kind::Org, "org").unwrap();

        let error = logout(&store, "nestodo.example.com").unwrap_err();

        assert_eq!(
            error.to_string(),
            "could not access the saved login: the keychain is locked"
        );
    }

    #[test]
    fn logout_still_forgets_the_app_token_when_the_long_lived_org_token_cannot_be_deleted() {
        let store = StuckToken(Kind::Org, MemoryStore::default());
        store.save("nestodo.example.com", Kind::App, "app").unwrap();
        store.save("nestodo.example.com", Kind::Org, "org").unwrap();

        logout(&store, "nestodo.example.com").unwrap_err();

        assert_eq!(store.load("nestodo.example.com", Kind::App).unwrap(), None);
    }
}
