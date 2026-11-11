use std::fmt;

#[derive(Debug)]
pub enum Error {
    Network { url: String, reason: String },
    NotSignedIn,
    Unauthorized { status: u16 },
    Access(String),
    Login(String),
    Cancelled,
    Invalid(String),
    Refused(Vec<(String, String)>),
    Server { status: u16, message: String },
    Storage(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Network { url, reason } => write!(f, "could not reach {url}: {reason}"),
            Error::NotSignedIn => write!(
                f,
                "you are not signed in or your session has expired; sign in again"
            ),
            Error::Unauthorized { status } => write!(
                f,
                "the server refused the request ({status}) and is not protected by Cloudflare Access, so signing in cannot help"
            ),
            Error::Access(reason) => write!(
                f,
                "could not verify the Cloudflare Access application: {reason}"
            ),
            Error::Login(reason) => write!(f, "sign-in failed: {reason}"),
            Error::Cancelled => write!(f, "sign-in was cancelled"),
            Error::Invalid(message) => write!(f, "{message}"),
            Error::Refused(fields) => {
                let messages: Vec<&str> =
                    fields.iter().map(|(_, message)| message.as_str()).collect();
                write!(f, "{}", messages.join("; "))
            }
            Error::Server { status, message } => {
                write!(f, "the server returned {status}: {message}")
            }
            Error::Storage(reason) => write!(f, "could not access the saved login: {reason}"),
        }
    }
}

impl std::error::Error for Error {}
