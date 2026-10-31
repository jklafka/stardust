//! The best score, kept in `~/.stardust_best`.

use std::fs;
use std::path::PathBuf;

fn path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".stardust_best"))
}

pub fn load() -> u32 {
    path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0)
}

/// Failures are ignored: a lost best score should never interrupt the game.
pub fn save(best: u32) {
    let Some(path) = path() else { return };
    let tmp = path.with_extension("tmp");
    if fs::write(&tmp, best.to_string()).is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}
