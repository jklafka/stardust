//! A cell buffer that redraws only the rows that changed.

use std::fmt::Write;

/// Terminal columns a character takes up. Covers the characters the game draws:
/// ASCII and the emoji that are always double width.
pub fn char_width(ch: char) -> usize {
    match ch as u32 {
        0x26AB | 0x2728 | 0x2B50 | 0x1F300..=0x1FAFF => 2,
        _ => 1,
    }
}

pub fn text_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

/// Marks the right half of a double-width character.
const CONTINUATION: char = '\0';

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<u8>,
    pub bg: Option<u8>,
    pub bold: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Cell {
    ch: char,
    style: Style,
}

const BLANK: Cell = Cell {
    ch: ' ',
    style: Style {
        fg: None,
        bg: None,
        bold: false,
    },
};

pub struct Screen {
    width: usize,
    height: usize,
    cells: Vec<Vec<Cell>>,
    previous: Vec<Option<Vec<Cell>>>,
}

impl Screen {
    pub fn new(width: usize, height: usize) -> Self {
        Screen {
            width,
            height,
            cells: vec![vec![BLANK; width]; height],
            previous: vec![None; height],
        }
    }

    pub fn clear(&mut self) {
        for row in &mut self.cells {
            row.fill(BLANK);
        }
    }

    fn set(&mut self, x: usize, y: usize, cell: Cell) {
        let row = &mut self.cells[y];
        if row[x].ch == CONTINUATION && x > 0 {
            row[x - 1].ch = ' '; // Overwriting the right half of a wide glyph.
        }
        if char_width(row[x].ch) == 2 && x + 1 < self.width {
            row[x + 1].ch = ' '; // Overwriting its left half.
        }
        row[x] = cell;
    }

    pub fn put(&mut self, x: i32, y: i32, text: &str, style: Style) {
        if y < 0 || y as usize >= self.height {
            return;
        }
        let y = y as usize;
        let mut x = x;
        for ch in text.chars() {
            let width = char_width(ch) as i32;
            if x >= 0 && (x + width) as usize <= self.width {
                self.set(x as usize, y, Cell { ch, style });
                if width == 2 {
                    self.set(
                        x as usize + 1,
                        y,
                        Cell {
                            ch: CONTINUATION,
                            style,
                        },
                    );
                }
            }
            x += width;
        }
    }

    pub fn fill(&mut self, x: i32, y: i32, w: usize, h: usize, bg: u8) {
        let blank = " ".repeat(w);
        let style = Style {
            bg: Some(bg),
            ..Style::default()
        };
        for row in y..y + h as i32 {
            self.put(x, row, &blank, style);
        }
    }

    /// Returns the escape sequences that bring the terminal up to date.
    pub fn render(&mut self) -> String {
        let mut out = String::new();
        for y in 0..self.height {
            if self.previous[y].as_ref() == Some(&self.cells[y]) {
                continue;
            }
            let _ = write!(out, "\x1b[{};1H", y + 1);
            let mut current = None;
            for (x, cell) in self.cells[y].iter().enumerate() {
                if cell.ch == CONTINUATION {
                    continue;
                }
                if current != Some(cell.style) {
                    current = Some(cell.style);
                    out.push_str("\x1b[0");
                    if cell.style.bold {
                        out.push_str(";1");
                    }
                    if let Some(fg) = cell.style.fg {
                        let _ = write!(out, ";38;5;{fg}");
                    }
                    if let Some(bg) = cell.style.bg {
                        let _ = write!(out, ";48;5;{bg}");
                    }
                    out.push('m');
                }
                out.push(cell.ch);
                if char_width(cell.ch) == 2 {
                    // Reposition so a font's unusual emoji width can't shift the row.
                    let _ = write!(out, "\x1b[{};{}H", y + 1, x + 3);
                }
            }
            out.push_str("\x1b[0m");
            self.previous[y] = Some(self.cells[y].clone());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() -> Style {
        Style::default()
    }

    #[test]
    fn every_tile_glyph_is_double_width() {
        for theme in &stardust_core::THEMES {
            for tier in &theme.tiers {
                assert_eq!(char_width(tier.glyph), 2, "{} {}", theme.id, tier.name);
            }
        }
    }

    #[test]
    fn overwriting_half_a_wide_glyph_clears_the_other_half() {
        let mut screen = Screen::new(6, 1);
        screen.put(1, 0, "✨", plain());
        screen.put(2, 0, "x", plain());
        assert_eq!(screen.cells[0][1].ch, ' ');

        let mut screen = Screen::new(6, 1);
        screen.put(1, 0, "✨", plain());
        screen.put(1, 0, "x", plain());
        assert_eq!(screen.cells[0][2].ch, ' ');
    }

    #[test]
    fn unchanged_rows_are_not_redrawn() {
        let mut screen = Screen::new(4, 2);
        screen.put(0, 0, "a", plain());
        assert!(screen.render().contains('a'));
        assert_eq!(screen.render(), "");
        screen.put(0, 1, "b", plain());
        let output = screen.render();
        assert!(output.contains('b'));
        assert!(!output.contains('a'));
    }

    #[test]
    fn drawing_off_screen_is_ignored() {
        let mut screen = Screen::new(2, 1);
        screen.put(-1, 0, "ab", plain());
        screen.put(0, 5, "c", plain());
        screen.put(1, 0, "✨", plain());
        assert_eq!(screen.cells[0][0].ch, 'b');
        assert_eq!(screen.cells[0][1].ch, ' ');
    }
}
