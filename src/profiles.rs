//! Saved nickname profiles: a small list kept in this mod's own, sandboxed config storage
//! (`ClientHost::config_files()`, `config/account_switcher/` in the game folder -- never the
//! shared `horizon-player.txt`, which `player_name.rs` owns), so the player can switch between
//! them from the UI without retyping a name each time, and always get back to the one they
//! started with.
//!
//! ## Why "hashed", and what that does and does not mean here
//! A Minecraft nickname is never a secret: every player on a server sees it in plain text, and it
//! is already stored unencrypted by the game itself (`horizon-player.txt`, `servers.dat`'s player
//! sample, server logs...). Hashing it cannot add confidentiality that does not exist to begin
//! with, and a one-way hash alone could not work anyway: switching back to a saved profile needs
//! the literal name again, which a pure hash can never recover. What this module *does* do is tag
//! each saved profile with a SHA-256 checksum of its fields (see `sha256.rs`) and refuse to load a
//! profile whose checksum does not match: a profile file that was hand-edited or corrupted on disk
//! is dropped instead of silently trusted.

use mod_api::{ClientHost, ConfigPath};

use crate::sha256;

/// One saved nickname.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    /// The literal offline nickname written to `horizon-player.txt` when this profile is active.
    pub name: String,
    /// A free-form label shown in the list; defaults to `name`.
    pub label: String,
    /// The profile recorded for the nickname already active the first time this mod ran: shown
    /// first, cannot be deleted, so the player can always revert to it with one click and no
    /// extra confirmation.
    pub main: bool,
    /// `host.millis()` at creation, for display and stable ordering only.
    pub created_at_ms: u64,
}

const STORE_FILE: &str = "profiles.txt";

impl Profile {
    fn checksum(&self) -> String {
        sha256::hex(
            format!(
                "account_switcher:v1:{}:{}:{}:{}",
                self.name, self.label, self.main, self.created_at_ms
            )
            .as_bytes(),
        )
    }

    fn to_block(&self) -> String {
        format!(
            "name={}\nlabel={}\nmain={}\ncreated={}\nhash={}\n",
            escape(&self.name),
            escape(&self.label),
            self.main as u8,
            self.created_at_ms,
            self.checksum()
        )
    }

    fn from_block(block: &str) -> Option<Self> {
        let mut name = None;
        let mut label = None;
        let mut main = None;
        let mut created_at_ms = None;
        let mut hash = None;
        for line in block.lines() {
            let (key, value) = line.split_once('=')?;
            match key {
                "name" => name = Some(unescape(value)),
                "label" => label = Some(unescape(value)),
                "main" => main = Some(value == "1"),
                "created" => created_at_ms = value.parse::<u64>().ok(),
                "hash" => hash = Some(value.to_owned()),
                _ => {}
            }
        }
        let profile = Self {
            name: name?,
            label: label?,
            main: main?,
            created_at_ms: created_at_ms?,
        };
        // A profile whose checksum does not match its own fields was hand-edited or corrupted:
        // drop it rather than trust it (see the module docs).
        (hash? == profile.checksum()).then_some(profile)
    }
}

/// `=` cannot appear in a value (it is the field separator); a literal `\` or `=` in a name or
/// label is escaped so the block format stays one key-value pair per line.
fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('=', "\\e")
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('e') => out.push('='),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn path() -> ConfigPath {
    ConfigPath::new(STORE_FILE).expect("a plain file name is always a valid ConfigPath")
}

/// Every saved profile, newest-created last, with `main` (if present) always first. A missing or
/// unreadable store (first run, or a wiped config folder) is just an empty list, not an error.
pub fn load(host: &dyn ClientHost) -> Vec<Profile> {
    let bytes = match host.config_files().read(&path()) {
        Ok(bytes) => bytes,
        Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut profiles: Vec<Profile> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty())
        .filter_map(Profile::from_block)
        .collect();
    profiles.sort_by_key(|p| (!p.main, p.created_at_ms));
    profiles
}

/// Replaces the saved list. `false` means the write failed (a full disk, a removed config
/// folder...); the caller still has its in-memory list for this session.
pub fn save(host: &dyn ClientHost, profiles: &[Profile]) -> bool {
    let body = profiles
        .iter()
        .map(Profile::to_block)
        .collect::<Vec<_>>()
        .join("\n");
    host.config_files().write(&path(), body.as_bytes()).is_ok()
}
