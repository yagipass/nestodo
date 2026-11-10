use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::error::Error;

const SERVICE: &str = "nestodo-desktop";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    App,
    Org,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::App => "app-token",
            Kind::Org => "org-token",
        }
    }
}

pub trait TokenStore: Send + Sync {
    fn load(&self, host: &str, kind: Kind) -> Result<Option<String>, Error>;
    fn save(&self, host: &str, kind: Kind, token: &str) -> Result<(), Error>;
    fn delete(&self, host: &str, kind: Kind) -> Result<(), Error>;
}

trait Keychain: Send + Sync {
    fn get(&self, account: &str) -> keyring::Result<Vec<u8>>;
    fn set(&self, account: &str, secret: &[u8]) -> keyring::Result<()>;
    fn delete(&self, account: &str) -> keyring::Result<()>;
}

struct OsKeychain;

impl Keychain for OsKeychain {
    fn get(&self, account: &str) -> keyring::Result<Vec<u8>> {
        keyring::Entry::new(SERVICE, account)?.get_secret()
    }

    fn set(&self, account: &str, secret: &[u8]) -> keyring::Result<()> {
        keyring::Entry::new(SERVICE, account)?.set_secret(secret)
    }

    fn delete(&self, account: &str) -> keyring::Result<()> {
        keyring::Entry::new(SERVICE, account)?.delete_credential()
    }
}

pub struct SystemStore {
    keychain: Box<dyn Keychain>,
    files: FileStore,
}

impl Default for SystemStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            keychain: Box::new(OsKeychain),
            files: FileStore {
                dir: dirs::config_dir().map(|dir| dir.join(SERVICE)),
            },
        }
    }

    fn account(host: &str, kind: Kind) -> String {
        format!("{host} {}", kind.name())
    }
}

impl TokenStore for SystemStore {
    fn load(&self, host: &str, kind: Kind) -> Result<Option<String>, Error> {
        let file = self.files.load(host, kind);
        if let Ok(Some(_)) = file {
            return file;
        }
        match self.keychain.get(&Self::account(host, kind)) {
            Ok(secret) => String::from_utf8(secret).map(Some).map_err(|_| {
                Error::Storage("the saved login is corrupted; sign out and sign in again".into())
            }),
            Err(_) => file,
        }
    }

    fn save(&self, host: &str, kind: Kind, token: &str) -> Result<(), Error> {
        match self
            .keychain
            .set(&Self::account(host, kind), token.as_bytes())
        {
            Ok(()) => {
                self.files.delete(host, kind)?;
                self.files.mark(host, kind)
            }
            Err(_) => self.files.save(host, kind, token),
        }
    }

    fn delete(&self, host: &str, kind: Kind) -> Result<(), Error> {
        let account = Self::account(host, kind);
        let deleted = self.keychain.delete(&account);
        self.files.delete(host, kind)?;
        match deleted {
            Ok(()) | Err(keyring::Error::NoEntry) => self.files.unmark(host, kind),
            Err(error)
                if self.files.is_marked(host, kind) || self.keychain.get(&account).is_ok() =>
            {
                Err(Error::Storage(error.to_string()))
            }
            _ => Ok(()),
        }
    }
}

struct FileStore {
    dir: Option<PathBuf>,
}

impl FileStore {
    fn path(&self, host: &str, kind: Kind) -> Result<PathBuf, Error> {
        let dir = self
            .dir
            .as_ref()
            .ok_or_else(|| Error::Storage("no keychain or config directory is available".into()))?;
        let host: String = host
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        Ok(dir.join(format!("{host}-{}", kind.name())))
    }

    fn marker(&self, host: &str, kind: Kind) -> Option<PathBuf> {
        let mut path = self.path(host, kind).ok()?.into_os_string();
        path.push(".keychain");
        Some(path.into())
    }

    fn load(&self, host: &str, kind: Kind) -> Result<Option<String>, Error> {
        match fs::read_to_string(self.path(host, kind)?) {
            Ok(token) => Ok(Some(token)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Error::Storage(error.to_string())),
        }
    }

    fn save(&self, host: &str, kind: Kind, token: &str) -> Result<(), Error> {
        Self::write(&self.path(host, kind)?, token)
    }

    fn mark(&self, host: &str, kind: Kind) -> Result<(), Error> {
        self.marker(host, kind)
            .map_or(Ok(()), |path| Self::write(&path, ""))
    }

    fn write(path: &Path, contents: &str) -> Result<(), Error> {
        let write = || -> io::Result<()> {
            let dir = path.parent().expect("token files live in a directory");
            let mut options = fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
                fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(dir)?;
                options.mode(0o600);
            }
            #[cfg(not(unix))]
            fs::create_dir_all(dir)?;
            let mut file = options.open(path)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(fs::Permissions::from_mode(0o600))?;
            }
            file.write_all(contents.as_bytes())
        };
        write().map_err(|error| Error::Storage(error.to_string()))
    }

    fn delete(&self, host: &str, kind: Kind) -> Result<(), Error> {
        self.path(host, kind)
            .map_or(Ok(()), |path| Self::remove(&path))
    }

    fn unmark(&self, host: &str, kind: Kind) -> Result<(), Error> {
        self.marker(host, kind)
            .map_or(Ok(()), |path| Self::remove(&path))
    }

    fn remove(path: &Path) -> Result<(), Error> {
        match fs::remove_file(path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                Err(Error::Storage(error.to_string()))
            }
            _ => Ok(()),
        }
    }

    fn is_marked(&self, host: &str, kind: Kind) -> bool {
        self.marker(host, kind)
            .is_some_and(|path| fs::exists(path).unwrap_or(true))
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore(std::sync::Mutex<std::collections::HashMap<(String, Kind), String>>);

#[cfg(test)]
impl TokenStore for MemoryStore {
    fn load(&self, host: &str, kind: Kind) -> Result<Option<String>, Error> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .get(&(host.to_string(), kind))
            .cloned())
    }

    fn save(&self, host: &str, kind: Kind, token: &str) -> Result<(), Error> {
        self.0
            .lock()
            .unwrap()
            .insert((host.to_string(), kind), token.to_string());
        Ok(())
    }

    fn delete(&self, host: &str, kind: Kind) -> Result<(), Error> {
        self.0.lock().unwrap().remove(&(host.to_string(), kind));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use super::*;

    struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct TempStore<T> {
        store: T,
        _dir: TempDir,
    }

    impl<T> std::ops::Deref for TempStore<T> {
        type Target = T;

        fn deref(&self) -> &T {
            &self.store
        }
    }

    fn temp_store(name: &str) -> TempStore<FileStore> {
        let dir = std::env::temp_dir().join(format!(
            "nestodo-desktop-test-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        TempStore {
            store: FileStore {
                dir: Some(dir.join(SERVICE)),
            },
            _dir: TempDir(dir),
        }
    }

    #[test]
    fn keeps_each_host_and_kind_apart_and_forgets_them_on_delete() {
        let store = temp_store("roundtrip");
        store.save("nestodo.example.com", Kind::App, "app").unwrap();
        store.save("nestodo.example.com", Kind::Org, "org").unwrap();
        assert_eq!(
            store
                .load("nestodo.example.com", Kind::App)
                .unwrap()
                .as_deref(),
            Some("app")
        );
        assert_eq!(
            store
                .load("nestodo.example.com", Kind::Org)
                .unwrap()
                .as_deref(),
            Some("org")
        );
        assert_eq!(store.load("other.example.com", Kind::App).unwrap(), None);

        store.delete("nestodo.example.com", Kind::App).unwrap();
        store.delete("nestodo.example.com", Kind::App).unwrap();
        assert_eq!(store.load("nestodo.example.com", Kind::App).unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn writes_tokens_that_only_the_owner_can_read() {
        use std::os::unix::fs::PermissionsExt;
        let store = temp_store("mode");
        let path = store.path("nestodo.example.com", Kind::App).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        store.save("nestodo.example.com", Kind::App, "new").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    }

    struct BrokenKeychain {
        still_holds: Option<String>,
    }

    impl Keychain for BrokenKeychain {
        fn get(&self, account: &str) -> keyring::Result<Vec<u8>> {
            if self.still_holds.as_deref() == Some(account) {
                Ok(Vec::new())
            } else {
                Err(locked())
            }
        }

        fn set(&self, _: &str, _: &[u8]) -> keyring::Result<()> {
            Err(locked())
        }

        fn delete(&self, _: &str) -> keyring::Result<()> {
            Err(locked())
        }
    }

    fn locked() -> keyring::Error {
        keyring::Error::NoStorageAccess("the keychain is locked".into())
    }

    fn system_store(name: &str, keychain: impl Keychain + 'static) -> TempStore<SystemStore> {
        let TempStore {
            store: files,
            _dir: dir,
        } = temp_store(name);
        TempStore {
            store: SystemStore {
                keychain: Box::new(keychain),
                files,
            },
            _dir: dir,
        }
    }

    #[derive(Default)]
    struct MemoryKeychain(Mutex<HashMap<String, Vec<u8>>>);

    impl Keychain for MemoryKeychain {
        fn get(&self, account: &str) -> keyring::Result<Vec<u8>> {
            self.0
                .lock()
                .unwrap()
                .get(account)
                .cloned()
                .ok_or(keyring::Error::NoEntry)
        }

        fn set(&self, account: &str, secret: &[u8]) -> keyring::Result<()> {
            self.0
                .lock()
                .unwrap()
                .insert(account.into(), secret.to_vec());
            Ok(())
        }

        fn delete(&self, account: &str) -> keyring::Result<()> {
            self.0
                .lock()
                .unwrap()
                .remove(account)
                .map(drop)
                .ok_or(keyring::Error::NoEntry)
        }
    }

    #[test]
    fn deletes_only_the_keychain_entry_asked_for_so_no_token_outlives_sign_out() {
        let store = system_store("keychain", MemoryKeychain::default());
        store.save("nestodo.example.com", Kind::App, "app").unwrap();
        store.save("nestodo.example.com", Kind::Org, "org").unwrap();

        store.delete("nestodo.example.com", Kind::App).unwrap();
        assert_eq!(store.load("nestodo.example.com", Kind::App).unwrap(), None);
        assert_eq!(
            store
                .load("nestodo.example.com", Kind::Org)
                .unwrap()
                .as_deref(),
            Some("org")
        );

        store.delete("nestodo.example.com", Kind::Org).unwrap();
        assert_eq!(store.load("nestodo.example.com", Kind::Org).unwrap(), None);
    }

    #[test]
    fn sign_out_also_removes_a_login_saved_to_files_so_the_next_launch_is_not_still_signed_in() {
        let store = system_store("both", MemoryKeychain::default());
        store
            .save("nestodo.example.com", Kind::Org, "desktop")
            .unwrap();
        store
            .files
            .save("nestodo.example.com", Kind::Org, "ssh")
            .unwrap();

        store.delete("nestodo.example.com", Kind::Org).unwrap();
        assert_eq!(store.load("nestodo.example.com", Kind::Org).unwrap(), None);
    }

    #[test]
    fn removes_the_older_token_file_on_a_keychain_login_so_a_locked_keychain_cannot_revive_it() {
        let store = system_store("stale", MemoryKeychain::default());
        store
            .files
            .save("nestodo.example.com", Kind::Org, "old")
            .unwrap();

        store.save("nestodo.example.com", Kind::Org, "new").unwrap();
        assert_eq!(
            store.files.load("nestodo.example.com", Kind::Org).unwrap(),
            None
        );
    }

    #[test]
    fn stays_signed_in_with_a_login_saved_to_files_once_the_keychain_can_be_used_again() {
        let store = system_store("unlocked", MemoryKeychain::default());
        store
            .files
            .save("nestodo.example.com", Kind::Org, "org")
            .unwrap();

        assert_eq!(
            store
                .load("nestodo.example.com", Kind::Org)
                .unwrap()
                .as_deref(),
            Some("org")
        );
    }

    #[test]
    fn signs_out_of_a_login_that_was_saved_to_files_because_the_keychain_could_not_be_used() {
        let store = system_store("locked", BrokenKeychain { still_holds: None });
        store.save("nestodo.example.com", Kind::App, "app").unwrap();
        store.save("nestodo.example.com", Kind::Org, "org").unwrap();
        assert_eq!(
            store
                .load("nestodo.example.com", Kind::Org)
                .unwrap()
                .as_deref(),
            Some("org")
        );

        store.delete("nestodo.example.com", Kind::App).unwrap();
        store.delete("nestodo.example.com", Kind::Org).unwrap();
        store.delete("nestodo.example.com", Kind::Org).unwrap();
        assert_eq!(store.load("nestodo.example.com", Kind::App).unwrap(), None);
        assert_eq!(store.load("nestodo.example.com", Kind::Org).unwrap(), None);
    }

    #[test]
    fn says_the_sign_out_failed_only_for_the_token_the_keychain_still_hands_out() {
        let store = system_store(
            "undeletable",
            BrokenKeychain {
                still_holds: Some(SystemStore::account("nestodo.example.com", Kind::Org)),
            },
        );
        store
            .files
            .save("nestodo.example.com", Kind::Org, "org")
            .unwrap();

        store.delete("nestodo.example.com", Kind::App).unwrap();
        assert!(matches!(
            store.delete("nestodo.example.com", Kind::Org),
            Err(Error::Storage(_))
        ));
        assert_eq!(
            store.files.load("nestodo.example.com", Kind::Org).unwrap(),
            None
        );
    }

    #[derive(Default)]
    struct LockableKeychain {
        items: MemoryKeychain,
        locked: Arc<AtomicBool>,
    }

    impl LockableKeychain {
        fn open(&self) -> keyring::Result<&MemoryKeychain> {
            if self.locked.load(Ordering::SeqCst) {
                Err(locked())
            } else {
                Ok(&self.items)
            }
        }
    }

    impl Keychain for LockableKeychain {
        fn get(&self, account: &str) -> keyring::Result<Vec<u8>> {
            self.open()?.get(account)
        }

        fn set(&self, account: &str, secret: &[u8]) -> keyring::Result<()> {
            self.open()?.set(account, secret)
        }

        fn delete(&self, account: &str) -> keyring::Result<()> {
            self.open()?.delete(account)
        }
    }

    #[test]
    fn sign_out_fails_while_a_locked_keychain_holds_the_login_so_unlocking_cannot_sign_back_in() {
        let keychain = LockableKeychain::default();
        let lock = Arc::clone(&keychain.locked);
        let store = system_store("relock", keychain);
        store.save("nestodo.example.com", Kind::Org, "A").unwrap();

        lock.store(true, Ordering::SeqCst);
        let sign_out = store.delete("nestodo.example.com", Kind::Org);
        assert!(matches!(sign_out, Err(Error::Storage(_))), "{sign_out:?}");

        lock.store(false, Ordering::SeqCst);
        store.delete("nestodo.example.com", Kind::Org).unwrap();
        assert_eq!(store.load("nestodo.example.com", Kind::Org).unwrap(), None);

        lock.store(true, Ordering::SeqCst);
        store.delete("nestodo.example.com", Kind::Org).unwrap();
    }

    #[test]
    fn a_login_saved_while_the_keychain_is_locked_beats_the_older_account_once_it_unlocks() {
        let keychain = LockableKeychain::default();
        let lock = Arc::clone(&keychain.locked);
        let store = system_store("switch", keychain);
        store.save("nestodo.example.com", Kind::Org, "A").unwrap();

        lock.store(true, Ordering::SeqCst);
        store.save("nestodo.example.com", Kind::Org, "B").unwrap();

        lock.store(false, Ordering::SeqCst);
        assert_eq!(
            store
                .load("nestodo.example.com", Kind::Org)
                .unwrap()
                .as_deref(),
            Some("B")
        );
    }

    struct EmptyKeychain;

    impl Keychain for EmptyKeychain {
        fn get(&self, _: &str) -> keyring::Result<Vec<u8>> {
            unreachable!("each keychain search can ask to unlock it")
        }

        fn set(&self, _: &str, _: &[u8]) -> keyring::Result<()> {
            Ok(())
        }

        fn delete(&self, _: &str) -> keyring::Result<()> {
            Err(keyring::Error::NoEntry)
        }
    }

    #[test]
    fn sign_out_does_not_search_the_keychain_again_for_a_token_it_says_is_not_there() {
        let store = system_store("absent", EmptyKeychain);
        store
            .files
            .save("nestodo.example.com", Kind::App, "app")
            .unwrap();

        store.delete("nestodo.example.com", Kind::App).unwrap();
        store.delete("nestodo.example.com", Kind::Org).unwrap();

        assert_eq!(
            store.files.load("nestodo.example.com", Kind::App).unwrap(),
            None
        );
    }

    #[test]
    fn cannot_escape_the_token_directory_through_the_host_name() {
        let store = temp_store("escape");
        let path = store.path("../../etc/passwd", Kind::App).unwrap();
        assert_eq!(path.parent(), store.dir.as_deref());
    }
}
