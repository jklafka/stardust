//! The chosen theme, kept in `~/.stardust_theme` as the plain theme id.

use std::fs;
use std::path::PathBuf;

use stardust_core::{THEMES, theme_index};

fn path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".stardust_theme"))
}

/// A theme index from the file's text. Missing or unknown ids mean the default.
fn parse(text: &str) -> usize {
    theme_index(text.trim()).unwrap_or(0)
}

pub fn load() -> usize {
    path()
        .and_then(|path| fs::read_to_string(path).ok())
        .map_or(0, |text| parse(&text))
}

/// Failures are ignored: a lost theme choice should never interrupt the game.
pub fn save(index: usize) {
    let Some(path) = path() else { return };
    let Some(theme) = THEMES.get(index) else {
        return;
    };
    let tmp = path.with_extension("tmp");
    if fs::write(&tmp, theme.id).is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ids_and_falls_back_to_space() {
        assert_eq!(parse("plants"), 1);
        assert_eq!(parse("plants\n"), 1);
        assert_eq!(parse("fantasy"), 2);
        assert_eq!(parse("space"), 0);
        assert_eq!(parse(""), 0);
        assert_eq!(parse("Plants"), 0);
        assert_eq!(parse("garbage"), 0);
    }
}
