//! Pixel art for the themes the web version draws as sprites, in the style of
//! SNES-era sprites: 16x24 pixels and at most 15 colors plus transparency, as a
//! SNES sprite had. Only the web version draws these. The terminal's tiles are
//! too small, so it uses each tier's emoji instead.
//!
//! Each theme's sprites live in their own file. In each drawing, `.` is
//! transparent and every other character is a palette entry.

mod fantasy;
mod scale;

pub const WIDTH: usize = 16;
pub const HEIGHT: usize = 24;

pub struct Sprite {
    /// The tier this draws, so a test can catch sprites in the wrong order.
    #[cfg_attr(not(test), expect(dead_code))]
    pub name: &'static str,
    pub palette: &'static [(char, u32)],
    pub rows: [&'static str; HEIGHT],
}

impl Sprite {
    /// A pixel as 0xRRGGBB, or None if it's transparent or off the sprite.
    pub fn pixel(&self, x: usize, y: usize) -> Option<u32> {
        let wanted = *self.rows.get(y)?.as_bytes().get(x)? as char;
        self.palette
            .iter()
            .find(|(ch, _)| *ch == wanted)
            .map(|(_, rgb)| *rgb)
    }
}

/// Every theme drawn with sprites, by theme id. Each list is in tier order,
/// lowest first.
const THEMED: [(&str, &[Sprite]); 2] = [("scale", &scale::SCALE), ("fantasy", &fantasy::FANTASY)];

/// A theme's sprites, or None if the theme is drawn with its glyphs.
pub fn for_theme(id: &str) -> Option<&'static [Sprite]> {
    THEMED
        .iter()
        .find(|(theme, _)| *theme == id)
        .map(|(_, sprites)| *sprites)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn every_sprite() -> impl Iterator<Item = &'static Sprite> {
        THEMED.iter().flat_map(|(_, sprites)| sprites.iter())
    }

    #[test]
    fn every_sprite_has_the_right_shape() {
        for sprite in every_sprite() {
            for row in &sprite.rows {
                assert_eq!(row.len(), WIDTH, "{}: {row}", sprite.name);
            }
        }
    }

    #[test]
    fn sprites_keep_to_their_palettes() {
        for sprite in every_sprite() {
            // SNES sprites had 15 colors plus transparency.
            assert!(sprite.palette.len() <= 15, "{}", sprite.name);
            let palette: HashSet<char> = sprite.palette.iter().map(|(ch, _)| *ch).collect();
            assert_eq!(palette.len(), sprite.palette.len(), "{}", sprite.name);
            assert!(!palette.contains(&'.'), "{}", sprite.name);

            let used: HashSet<char> = sprite.rows.iter().flat_map(|row| row.chars()).collect();
            for ch in used.iter().filter(|ch| **ch != '.') {
                assert!(palette.contains(ch), "{} uses {ch}", sprite.name);
            }
            for ch in &palette {
                assert!(used.contains(ch), "{} never uses {ch}", sprite.name);
            }
        }
    }

    #[test]
    fn themes_are_listed_once() {
        let ids: HashSet<&str> = THEMED.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids.len(), THEMED.len());
        assert!(for_theme("scale").is_some());
        assert!(for_theme("fantasy").is_some());
        assert!(for_theme("nope").is_none());
    }

    #[test]
    fn pixels_read_from_the_drawing() {
        let bard = &fantasy::FANTASY[0];
        assert_eq!(bard.pixel(0, 0), None); // transparent
        assert_eq!(bard.pixel(9, 0), Some(0x1c1424));
        assert_eq!(bard.pixel(WIDTH, 0), None);
        assert_eq!(bard.pixel(0, HEIGHT), None);
    }
}
