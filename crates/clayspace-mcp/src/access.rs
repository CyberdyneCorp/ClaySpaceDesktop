//! Who may come in, and how they learn where the door is.
//!
//! The door is on loopback, so the boundary that protects it is the one the
//! filesystem already draws: a file only this user can read, holding the port
//! and a secret that is new for every run. That is the same boundary already
//! protecting the autosave and the recovery marker, and it is stated rather
//! than implied — **any process running as this user can read that file and
//! drive the session**, which is exactly why the operations that can destroy
//! work need a consent this file cannot supply.

use std::io::Read;
use std::path::{Path, PathBuf};

/// The file in the session directory that publishes the door.
///
/// Portuguese, like `sessão.aberta` and `referências.txt` beside it. A second
/// naming convention in one directory is a directory nobody can predict.
pub const ACCESS_FILE: &str = "agente.acesso";

/// The file recording which kinds of gated operation the person has agreed to
/// once and for all, one tag per line.
pub const CONSENT_FILE: &str = "agente.consentimentos";

/// The file recording whether the door was shut by hand.
pub const DOOR_FILE: &str = "agente.porta";

/// The port the server asks for first.
///
/// Nothing else is known to want it, and a fixed number means a client's
/// configuration usually needs writing once. Where it is taken, the server
/// takes another and publishes what it took.
pub const PREFERRED_PORT: u16 = 7457;

/// How many ports past the preferred one to try before giving up.
pub const PORT_ATTEMPTS: u16 = 32;

/// What a client needs to reach this session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Access {
    pub port: u16,
    pub secret: String,
    pub pid: u32,
}

impl Access {
    /// The URL a client is configured with.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }

    /// The file's contents. Lines of `key value`, which is what the session
    /// directory's other files are.
    pub fn to_text(&self) -> String {
        format!(
            "porta {}\nchave {}\nprocesso {}\n",
            self.port, self.secret, self.pid
        )
    }

    pub fn from_text(text: &str) -> Option<Self> {
        let mut port = None;
        let mut secret = None;
        let mut pid = None;
        for line in text.lines() {
            let mut words = line.splitn(2, ' ');
            match (words.next(), words.next()) {
                (Some("porta"), Some(value)) => port = value.trim().parse().ok(),
                (Some("chave"), Some(value)) => secret = Some(value.trim().to_string()),
                (Some("processo"), Some(value)) => pid = value.trim().parse().ok(),
                _ => {}
            }
        }
        Some(Self {
            port: port?,
            secret: secret?,
            pid: pid.unwrap_or(0),
        })
    }

    /// Writes the file so that only its owner can read it, and returns where.
    ///
    /// The permissions are set *before* the secret is written, not after: a
    /// file created world-readable and tightened a moment later is a file
    /// something else can read in that moment.
    pub fn publish(&self, root: &Path) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(root)?;
        let path = root.join(ACCESS_FILE);
        write_private(&path, self.to_text().as_bytes())?;
        Ok(path)
    }

    /// Takes the file away, which is what makes a secret read once useless
    /// against the next session.
    pub fn withdraw(root: &Path) {
        let _ = std::fs::remove_file(root.join(ACCESS_FILE));
    }
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.flush()
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}

/// A secret with 256 bits behind it, as lowercase hex.
///
/// Read from the operating system's own pool. There is no fallback to a clock
/// and a process id: a door with a guessable secret is worse than a door that
/// did not open, because only one of the two is obvious from outside.
pub fn generate_secret() -> std::io::Result<String> {
    let mut bytes = [0u8; 32];
    let mut file = std::fs::File::open("/dev/urandom")?;
    file.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Compares two secrets without letting the time taken say how much of one was
/// right.
pub fn secret_matches(expected: &str, offered: &str) -> bool {
    let expected = expected.as_bytes();
    let offered = offered.as_bytes();
    // The length of our own secret is a constant and is not a secret.
    if expected.len() != offered.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in expected.iter().zip(offered.iter()) {
        difference |= a ^ b;
    }
    difference == 0
}

/// The secret an `Authorization` header offers, if it offers one.
pub fn bearer(header: Option<&str>) -> Option<&str> {
    let header = header?;
    let (scheme, token) = header.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

/// Whether a request's declared origin and host are ones this server issued.
///
/// A native client sends neither, and that is the ordinary case. What this
/// refuses is the other one: a page in a browser reaching a loopback server
/// through a name that resolves to it, which is the attack a local HTTP
/// server has that a Unix socket does not.
pub fn origin_is_ours(origin: Option<&str>, host: Option<&str>, port: u16) -> bool {
    if let Some(origin) = origin {
        let ours = [
            format!("http://127.0.0.1:{port}"),
            format!("http://localhost:{port}"),
            format!("http://[::1]:{port}"),
        ];
        if !ours.iter().any(|ours| ours == origin) {
            return false;
        }
    }
    if let Some(host) = host {
        let ours = [
            format!("127.0.0.1:{port}"),
            format!("localhost:{port}"),
            format!("[::1]:{port}"),
        ];
        if !ours.iter().any(|ours| ours == host) {
            return false;
        }
    }
    true
}

/// The kinds of gated operation the person has agreed to once and for all.
pub fn read_consents(root: &Path) -> Vec<String> {
    std::fs::read_to_string(root.join(CONSENT_FILE))
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn write_consents(root: &Path, tags: &[String]) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    let mut text = String::new();
    for tag in tags {
        text.push_str(tag);
        text.push('\n');
    }
    std::fs::write(root.join(CONSENT_FILE), text)
}

/// Whether the door was shut by hand. A door a person closed stays closed when
/// the application is opened again.
pub fn door_was_shut(root: &Path) -> bool {
    std::fs::read_to_string(root.join(DOOR_FILE))
        .map(|text| text.trim() == "fechada")
        .unwrap_or(false)
}

pub fn remember_door(root: &Path, open: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join(DOOR_FILE),
        if open { "aberta\n" } else { "fechada\n" },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("clayspace-mcp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn a_secret_is_thirty_two_bytes_of_hex_and_never_the_same_twice() {
        let one = generate_secret().unwrap();
        let two = generate_secret().unwrap();
        assert_eq!(one.len(), 64);
        assert!(one.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(one, two);
    }

    #[test]
    fn a_secret_matches_only_itself() {
        let secret = generate_secret().unwrap();
        assert!(secret_matches(&secret, &secret));
        assert!(!secret_matches(&secret, &secret[..63]));
        let mut wrong = secret.clone();
        wrong.replace_range(0..1, if secret.starts_with('a') { "b" } else { "a" });
        assert!(!secret_matches(&secret, &wrong));
        assert!(!secret_matches(&secret, ""));
    }

    #[test]
    fn the_access_file_round_trips() {
        let access = Access {
            port: 7457,
            secret: "abc".into(),
            pid: 42,
        };
        assert_eq!(Access::from_text(&access.to_text()), Some(access));
    }

    #[test]
    #[cfg(unix)]
    fn the_access_file_is_readable_by_its_owner_alone() {
        use std::os::unix::fs::PermissionsExt;

        let root = scratch("access");
        let access = Access {
            port: 7457,
            secret: generate_secret().unwrap(),
            pid: std::process::id(),
        };
        let path = access.publish(&root).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "{mode:o}");

        Access::withdraw(&root);
        assert!(!path.exists());
    }

    #[test]
    fn a_bearer_header_gives_up_its_token() {
        assert_eq!(bearer(Some("Bearer abc")), Some("abc"));
        assert_eq!(bearer(Some("bearer abc")), Some("abc"));
        assert_eq!(bearer(Some("Basic abc")), None);
        assert_eq!(bearer(Some("Bearer ")), None);
        assert_eq!(bearer(Some("abc")), None);
        assert_eq!(bearer(None), None);
    }

    #[test]
    fn a_native_client_sends_no_origin_and_is_served() {
        assert!(origin_is_ours(None, None, 7457));
        assert!(origin_is_ours(None, Some("127.0.0.1:7457"), 7457));
    }

    #[test]
    fn a_web_origin_is_not_one_we_issued() {
        assert!(!origin_is_ours(Some("https://example.test"), None, 7457));
        assert!(!origin_is_ours(Some("http://127.0.0.1:9999"), None, 7457));
        assert!(origin_is_ours(Some("http://127.0.0.1:7457"), None, 7457));
        assert!(origin_is_ours(Some("http://localhost:7457"), None, 7457));
    }

    #[test]
    fn a_host_that_is_not_loopback_is_a_rebinding_attempt() {
        assert!(!origin_is_ours(
            None,
            Some("sculpt.example.test:7457"),
            7457
        ));
    }

    #[test]
    fn consents_and_the_door_are_remembered() {
        let root = scratch("consents");
        assert!(read_consents(&root).is_empty());
        write_consents(&root, &["exportar".into(), "sobrescrever".into()]).unwrap();
        assert_eq!(read_consents(&root), vec!["exportar", "sobrescrever"]);

        assert!(!door_was_shut(&root));
        remember_door(&root, false).unwrap();
        assert!(door_was_shut(&root));
        remember_door(&root, true).unwrap();
        assert!(!door_was_shut(&root));
    }
}
