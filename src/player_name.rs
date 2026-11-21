//! Reads and writes `horizon-player.txt`, the file Horizon's own offline-name module
//! (`horizon_client::offline_name`, see the engine source cited in this mod's README) keeps next
//! to `options.txt`: the nickname a client joins offline-mode (cracked) servers with when it was
//! started with no `--name`, no `HORIZON_PLAYER_NAME` and no signed-in Microsoft account. The
//! engine re-reads this file fresh before every such connection, so writing a new name here is
//! picked up on the very next "Join Server" click -- no restart needed.
//!
//! This only ever changes what an **offline-mode** server sees: a signed-in account always joins
//! under its own, server-verified name regardless of this file's contents, exactly as vanilla and
//! Horizon already behave. An offline-mode nickname was never authenticated to begin with -- it is
//! already a plain, freely editable string before this mod touches it (the engine's own
//! `offline_name.rs` says so explicitly: "An edited name is used as it is").

use std::fs;
use std::path::PathBuf;

pub const FILE_NAME: &str = "horizon-player.txt";
const KEY: &str = "name:";

/// Vanilla's player-name rule (`StringUtil.isValidPlayerName`), checked again by the engine
/// itself when it next reads the file: 1-16 characters, letters, digits and underscore only.
/// This mod's own check is a UX convenience, not the authority -- if it is ever looser than the
/// engine's, the engine silently replaces an unusable name with a fresh generated one rather than
/// failing, so nothing can be bricked by a mismatch here.
pub fn is_valid_name(name: &str) -> bool {
    let len = name.chars().count();
    (1..=16).contains(&len)
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Best-effort folder for `horizon-player.txt`: the game's current working directory, which the
/// launcher sets to the instance folder before the process starts, same as `options.txt`. Horizon
/// itself prefers `--player-dir` first, when a player started a splitscreen session with one; that
/// flag cannot be read back from inside a mod, so this is the one case where this mod's "current
/// nickname" view and the engine's can briefly disagree.
fn base_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_default()
}

fn file_path() -> PathBuf {
    base_dir().join(FILE_NAME)
}

/// The nickname `horizon-player.txt` currently holds, if any and if it parses as a valid name
/// (mirrors the engine's own `parse`/`is_valid`, see the module docs).
pub fn read_active() -> Option<String> {
    let text = fs::read_to_string(file_path()).ok()?;
    text.lines()
        .find_map(|line| line.trim_end_matches('\r').strip_prefix(KEY))
        .filter(|name| is_valid_name(name))
        .map(str::to_owned)
}

/// Writes `name` into `horizon-player.txt`, in the same `name:<value>` form the engine writes and
/// parses. The caller validates `name` with [`is_valid_name`] first.
pub fn write_active(name: &str) -> std::io::Result<()> {
    fs::write(file_path(), format!("{KEY}{name}\n"))
}
