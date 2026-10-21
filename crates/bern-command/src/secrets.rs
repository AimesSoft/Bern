//! Secrets the application keeps for the user, such as a sign-in token, in
//! the system's credential store: the Keychain on macOS and iOS, the
//! Credential Manager on Windows, the Secret Service on Linux. They stay out
//! of the application's files, so copying or syncing those never carries a
//! secret along.
//!
//! Each secret is stored under the application's identity and a simple
//! name. Engines without a host, such as tests, keep their secrets in memory
//! for the life of the process. Where the system has no credential store
//! Bern can reach (tvOS, Android and the browser for now), reading finds
//! nothing and writing fails with `secrets_unavailable`.

use bern_protocol::CommandError;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Secrets of an engine without a host.
static MEMORY: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn memory() -> std::sync::MutexGuard<'static, HashMap<String, String>> {
    MEMORY
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn checked(name: &str) -> Result<(), CommandError> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(CommandError::new(
            "invalid_secret_name",
            "secret names are simple names",
        ))
    }
}

/// The secret saved as `name`, if there is one.
pub fn read(name: &str) -> Result<Option<String>, CommandError> {
    checked(name)?;
    match crate::storage::application_id() {
        Some(service) => system::read(service, name),
        None => Ok(memory().get(name).cloned()),
    }
}

/// Save `value` as the secret `name`, replacing what was saved before.
pub fn write(name: &str, value: &str) -> Result<(), CommandError> {
    checked(name)?;
    match crate::storage::application_id() {
        Some(service) => system::write(service, name, value),
        None => {
            memory().insert(name.into(), value.into());
            Ok(())
        }
    }
}

/// Forget the secret `name`; forgetting one that was never saved succeeds.
pub fn delete(name: &str) -> Result<(), CommandError> {
    checked(name)?;
    match crate::storage::application_id() {
        Some(service) => system::delete(service, name),
        None => {
            memory().remove(name);
            Ok(())
        }
    }
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
))]
fn failed(error: impl std::fmt::Display) -> CommandError {
    CommandError::new("secrets_failed", error.to_string())
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod system {
    use super::*;
    use security_framework::passwords;

    /// `errSecItemNotFound`.
    const NOT_FOUND: i32 = -25300;

    pub fn read(service: &str, name: &str) -> Result<Option<String>, CommandError> {
        match passwords::get_generic_password(service, name) {
            Ok(bytes) => String::from_utf8(bytes).map(Some).map_err(failed),
            Err(error) if error.code() == NOT_FOUND => Ok(None),
            Err(error) => Err(failed(error)),
        }
    }

    pub fn write(service: &str, name: &str, value: &str) -> Result<(), CommandError> {
        passwords::set_generic_password(service, name, value.as_bytes()).map_err(failed)
    }

    pub fn delete(service: &str, name: &str) -> Result<(), CommandError> {
        match passwords::delete_generic_password(service, name) {
            Err(error) if error.code() != NOT_FOUND => Err(failed(error)),
            _ => Ok(()),
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
mod system {
    use super::*;
    use keyring::{Entry, Error};

    pub fn read(service: &str, name: &str) -> Result<Option<String>, CommandError> {
        match Entry::new(service, name).and_then(|entry| entry.get_password()) {
            Ok(value) => Ok(Some(value)),
            Err(Error::NoEntry) => Ok(None),
            Err(error) => Err(failed(error)),
        }
    }

    pub fn write(service: &str, name: &str, value: &str) -> Result<(), CommandError> {
        Entry::new(service, name)
            .and_then(|entry| entry.set_password(value))
            .map_err(failed)
    }

    pub fn delete(service: &str, name: &str) -> Result<(), CommandError> {
        match Entry::new(service, name).and_then(|entry| entry.delete_credential()) {
            Ok(()) | Err(Error::NoEntry) => Ok(()),
            Err(error) => Err(failed(error)),
        }
    }
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
    target_os = "linux"
)))]
mod system {
    use super::*;

    fn unavailable() -> CommandError {
        CommandError::new(
            "secrets_unavailable",
            "this platform has no credential store Bern can use yet",
        )
    }

    pub fn read(_service: &str, _name: &str) -> Result<Option<String>, CommandError> {
        Ok(None)
    }

    pub fn write(_service: &str, _name: &str, _value: &str) -> Result<(), CommandError> {
        Err(unavailable())
    }

    pub fn delete(_service: &str, _name: &str) -> Result<(), CommandError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_engine_without_a_host_keeps_its_secrets_in_memory() {
        assert_eq!(read("token-test").unwrap(), None);
        write("token-test", "abc").unwrap();
        write("token-test", "def").unwrap();
        assert_eq!(read("token-test").unwrap().as_deref(), Some("def"));
        delete("token-test").unwrap();
        delete("token-test").unwrap();
        assert_eq!(read("token-test").unwrap(), None);
        assert_eq!(
            write("../token", "x").unwrap_err().code,
            "invalid_secret_name"
        );
    }
}
