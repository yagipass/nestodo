use std::fmt;

#[derive(Debug)]
pub enum Error {
    Network { url: String, reason: String },
    NotSignedIn,
    NotProtected { host: String },
    Unauthorized { status: u16 },
    Access(String),
    Login(String),
    NotFound { id: u64 },
    Invalid(String),
    Server { status: u16, message: String },
    Storage(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Network { url, reason } => write!(f, "could not reach {url}: {reason}"),
            Error::NotSignedIn => write!(
                f,
                "you are not signed in or your session has expired; run `nestodo login`"
            ),
            Error::NotProtected { host } => write!(
                f,
                "{host} is not protected by Cloudflare Access, so there is nothing to log in to"
            ),
            Error::Unauthorized { status } => write!(
                f,
                "the server refused the request ({status}) and is not protected by Cloudflare Access, so `nestodo login` cannot help"
            ),
            Error::Access(reason) => write!(
                f,
                "could not verify the Cloudflare Access application: {reason}"
            ),
            Error::Login(reason) => write!(f, "login failed: {reason}"),
            Error::NotFound { id } => {
                write!(f, "todo {id} not found; run `nestodo list` to see the IDs")
            }
            Error::Invalid(message) => write!(f, "{message}"),
            Error::Server { status, message } => {
                write!(f, "the server returned {status}: {message}")
            }
            Error::Storage(reason) => write!(f, "could not access the saved login: {reason}"),
        }
    }
}

impl std::error::Error for Error {}
