//! Draws the game and connects it to player input.

use std::io::ErrorKind;

use crate::best;
use crate::input::{Event, parse_input, swipe_direction};
use crate::screen::{Screen, Style, text_width};
use crate::terminal;
use stardust_core::{Cell, Direction, Game, MoveResult, SIZE, TIERS, tier_of};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub tile_w: usize,
    pub tile_h: usize,
    pub gap_x: usize,
    pub gap_y: usize,
    pub pad_x: usize,
    pub pad_y: usize,
    pub panel: bool,
    pub width: usize,
    pub height: usize,
}

impl Layout {
    pub const fn board_w(&self) -> usize {
        SIZE * self.tile_w + (SIZE - 1) * self.gap_x + 2 * self.pad_x
    }

    pub const fn board_h(&self) -> usize {
        SIZE * self.tile_h + (SIZE - 1) * self.gap_y + 2 * self.pad_y
    }
}

pub const WIDE: Layout = Layout {
    tile_w: 10,
    tile_h: 5,
    gap_x: 2,
    gap_y: 1,
    pad_x: 2,
    pad_y: 0,
    panel: true,
    width: 71,
    height: 24,
};

/// For phones and other narrow terminals.
pub const COMPACT: Layout = Layout {
    tile_w: 10,
    tile_h: 5,
    gap_x: 2,
    gap_y: 1,
    pad_x: 0,
    pad_y: 0,
    panel: false,
    width: 46,
    height: 25,
};

pub fn choose_layout(width: usize, height: usize) -> Option<Layout> {
    [WIDE, COMPACT]
        .into_iter()
        .find(|layout| width >= layout.width && height >= layout.height)
}

const BOARD_BG: u8 = 235;
const EMPTY_BG: u8 = 238;
const OVERLAY_BG: u8 = 17;
const ACCENT: u8 = 220;
const MUTED: u8 = 245;
const BRIGHT: u8 = 255;

const fn fg(color: u8) -> Style {
    Style {
        fg: Some(color),
        bg: None,
        bold: false,
    }
}

const fn bold(color: u8) -> Style {
    Style {
        fg: Some(color),
        bg: None,
        bold: true,
    }
}

const SLIDE_STEPS: u32 = 9;
const SLIDE_FRAME_MS: u32 = 20;

pub struct App {
    pub game: Game,
    screen: Screen,
    layout: Option<Layout>,
    size: (usize, usize),
    origin: (i32, i32),
    press: Option<(i32, i32)>,
    /// More input is already waiting to be handled.
    queued: bool,
}

impl App {
    pub fn new(game: Game) -> Self {
        App {
            game,
            screen: Screen::new(0, 0),
            layout: None,
            size: (0, 0),
            origin: (0, 0),
            press: None,
            queued: false,
        }
    }

    /// Rebuilds the screen after a resize. Returns true if the size changed.
    fn check_size(&mut self) -> bool {
        let size = terminal::size();
        if size == self.size {
            return false;
        }
        self.size = size;
        self.screen = Screen::new(size.0, size.1);
        self.layout = choose_layout(size.0, size.1);
        if let Some(layout) = self.layout {
            self.origin = (
                (size.0 - layout.width) as i32 / 2,
                (size.1 - layout.height) as i32 / 2,
            );
        }
        terminal::write_out("\x1b[0m\x1b[2J");
        true
    }

    fn board_origin(&self, layout: &Layout) -> (i32, i32) {
        (
            self.origin.0,
            self.origin.1 + if layout.panel { 0 } else { 1 },
        )
    }

    /// Screen position of a board cell. Fractional cells are used mid-animation.
    fn cell_xy(&self, layout: &Layout, r: f32, c: f32) -> (i32, i32) {
        let (ox, oy) = self.board_origin(layout);
        let step_x = (layout.tile_w + layout.gap_x) as f32;
        let step_y = (layout.tile_h + layout.gap_y) as f32;
        (
            ox + layout.pad_x as i32 + (c * step_x).round() as i32,
            oy + layout.pad_y as i32 + (r * step_y).round() as i32,
        )
    }

    fn cell_pos(&self, layout: &Layout, (r, c): Cell) -> (i32, i32) {
        self.cell_xy(layout, r as f32, c as f32)
    }

    fn draw_tile(&mut self, layout: &Layout, (x, y): (i32, i32), rank: u8) {
        let tier = tier_of(rank);
        let (w, h) = (layout.tile_w as i32, layout.tile_h);
        let text = Style {
            fg: Some(tier.fg),
            bg: Some(tier.bg),
            bold: false,
        };
        let strong = Style { bold: true, ..text };
        self.screen.fill(x, y, w as usize, h, tier.bg);
        self.screen.put(x + 1, y, &rank.to_string(), strong);
        self.screen.put(
            x + (w - 2) / 2,
            y + 2,
            tier.glyph.encode_utf8(&mut [0; 4]),
            text,
        );
        let name_x = x + (w - text_width(tier.name) as i32) / 2;
        self.screen.put(name_x, y + 3, tier.name, strong);
    }

    fn draw_chrome(&mut self, layout: &Layout) {
        let (ox, oy) = self.origin;
        let (bx, by) = self.board_origin(layout);
        let (score, best) = (self.game.score, self.game.best);
        self.screen.clear();
        if layout.panel {
            let px = ox + layout.board_w() as i32 + 2;
            let highest = self.game.highest();
            self.screen.put(px, oy, "S T A R D U S T", bold(ACCENT));
            self.screen.put(px, oy + 2, "SCORE", fg(MUTED));
            self.screen.put(
                px + 7,
                oy + 2,
                &score.to_string(),
                Style {
                    bold: true,
                    ..Style::default()
                },
            );
            self.screen.put(px, oy + 3, "BEST", fg(MUTED));
            self.screen.put(
                px + 7,
                oy + 3,
                &best.to_string(),
                Style {
                    bold: true,
                    ..Style::default()
                },
            );
            self.screen
                .put(px, oy + 5, "Smallest to largest", fg(MUTED));
            for (i, tier) in TIERS.iter().enumerate() {
                let rank = i as u8 + 1;
                let row = oy + 6 + i as i32;
                let number = fg(if rank == highest { ACCENT } else { MUTED });
                self.screen.put(px, row, &format!("{rank:>2}"), number);
                self.screen.put(
                    px + 3,
                    row,
                    tier.glyph.encode_utf8(&mut [0; 4]),
                    Style::default(),
                );
                let name = Style {
                    fg: Some(if rank <= highest { BRIGHT } else { MUTED }),
                    bg: None,
                    bold: rank == highest,
                };
                self.screen.put(px + 6, row, tier.name, name);
            }
        } else {
            self.screen.put(ox, oy, "STARDUST", bold(ACCENT));
            let status = format!("Score {score}  Best {best}");
            let status_x = ox + layout.width as i32 - status.len() as i32;
            self.screen.put(status_x, oy, &status, Style::default());
        }
        let controls = "Arrows/WASD/IJKL/swipe: move  n: new  q: quit";
        let controls_x = ox + (layout.board_w() as i32 - controls.len() as i32) / 2;
        self.screen.put(
            controls_x,
            by + layout.board_h() as i32,
            controls,
            fg(MUTED),
        );
        self.screen
            .fill(bx, by, layout.board_w(), layout.board_h(), BOARD_BG);
        for r in 0..SIZE {
            for c in 0..SIZE {
                let (x, y) = self.cell_pos(layout, (r, c));
                self.screen
                    .fill(x, y, layout.tile_w, layout.tile_h, EMPTY_BG);
            }
        }
    }

    fn draw_overlay(&mut self, layout: &Layout, lines: &[(String, bool)]) {
        let (bx, by) = self.board_origin(layout);
        let widest = lines
            .iter()
            .map(|(text, _)| text_width(text))
            .max()
            .unwrap_or(0);
        let w = (layout.board_w() - 2).min(widest + 6) as i32;
        let h = lines.len() as i32 + 2;
        let x = bx + (layout.board_w() as i32 - w) / 2;
        let y = by + (layout.board_h() as i32 - h) / 2;
        self.screen.fill(x, y, w as usize, h as usize, OVERLAY_BG);
        for (i, (text, strong)) in lines.iter().enumerate() {
            let text_x = x + (w - text_width(text) as i32) / 2;
            let style = Style {
                fg: Some(if *strong { ACCENT } else { BRIGHT }),
                bg: Some(OVERLAY_BG),
                bold: *strong,
            };
            self.screen.put(text_x, y + 1 + i as i32, text, style);
        }
    }

    /// Paints the resting board.
    fn compose_board(&mut self, layout: &Layout) {
        self.draw_chrome(layout);
        for r in 0..SIZE {
            for c in 0..SIZE {
                let rank = self.game.board[r][c];
                if rank != 0 {
                    let position = self.cell_pos(layout, (r, c));
                    self.draw_tile(layout, position, rank);
                }
            }
        }
    }

    fn flush(&mut self) {
        terminal::write_out(&self.screen.render());
    }

    fn draw(&mut self) {
        let Some(layout) = self.layout else {
            let message = format!(
                "Enlarge the terminal to {}x{}",
                COMPACT.width, COMPACT.height
            );
            self.screen.clear();
            let x = self.size.0.saturating_sub(message.len()) as i32 / 2;
            self.screen
                .put(x, self.size.1 as i32 / 2, &message, Style::default());
            self.flush();
            return;
        };
        self.compose_board(&layout);
        let (score, best) = (self.game.score, self.game.best);
        if self.game.showing_win() {
            self.draw_overlay(
                &layout,
                &[
                    ("You made a Universe!".into(), true),
                    (format!("Score {score}"), false),
                    ("c: keep going".into(), false),
                    ("n: new game   q: quit".into(), false),
                ],
            );
        } else if self.game.over() {
            self.draw_overlay(
                &layout,
                &[
                    ("No moves left".into(), true),
                    (format!("Score {score}   Best {best}"), false),
                    ("n: new game   q: quit".into(), false),
                ],
            );
        }
        self.flush();
    }

    /// Shows a frame. Returns false if input arrived, which skips the animation.
    fn frame(&mut self, ms: u32) -> bool {
        self.flush();
        !terminal::input_ready(ms)
    }

    fn animate(&mut self, result: &MoveResult) {
        let Some(layout) = self.layout else { return };
        if self.queued {
            return;
        }
        for step in 1..=SLIDE_STEPS {
            let t = step as f32 / SLIDE_STEPS as f32;
            let t = 1.0 - (1.0 - t) * (1.0 - t); // Ease out.
            self.draw_chrome(&layout);
            for slide in &result.slides {
                let r = slide.from.0 as f32 + (slide.to.0 as f32 - slide.from.0 as f32) * t;
                let c = slide.from.1 as f32 + (slide.to.1 as f32 - slide.from.1 as f32) * t;
                let position = self.cell_xy(&layout, r, c);
                self.draw_tile(&layout, position, slide.rank);
            }
            if !self.frame(SLIDE_FRAME_MS) {
                return;
            }
        }
    }

    fn do_move(&mut self, direction: Direction) {
        if self.game.showing_win() || self.game.over() {
            return;
        }
        let best_before = self.game.best;
        let Some((result, _)) = self.game.try_move(direction) else {
            return;
        };
        if self.game.best != best_before {
            best::save(self.game.best);
        }
        self.animate(&result);
    }

    /// Returns false when the game should exit.
    fn handle(&mut self, event: Event) -> bool {
        match event {
            Event::Quit => return false,
            Event::New => self.game.reset(),
            Event::Continue => {
                if self.game.showing_win() {
                    self.game.continued = true;
                }
            }
            Event::Move(direction) => self.do_move(direction),
            Event::Press(x, y) => self.press = Some((x, y)),
            Event::Release(x, y) => {
                if let Some((x0, y0)) = self.press.take()
                    && let Some(direction) = swipe_direction(x0, y0, x, y)
                {
                    self.do_move(direction);
                }
            }
        }
        true
    }

    /// Runs until the player quits, the terminal closes, or a signal arrives.
    pub fn run(&mut self) {
        let mut pending = Vec::new();
        let mut buffer = [0; 1024];
        let mut dirty = true;
        loop {
            if terminal::pending_signal().is_some() {
                return;
            }
            if self.check_size() {
                dirty = true;
            }
            if dirty {
                self.draw();
                dirty = false;
            }
            if !terminal::input_ready(100) {
                pending.clear(); // A lone Escape key; no sequence is coming.
                continue;
            }
            let count = match terminal::read_input(&mut buffer) {
                Ok(0) => return,
                Ok(count) => count,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return,
            };
            pending.extend_from_slice(&buffer[..count]);
            let (events, rest) = parse_input(&pending);
            pending = rest;
            let total = events.len();
            for (i, event) in events.into_iter().enumerate() {
                self.queued = i + 1 < total;
                if !self.handle(event) {
                    return;
                }
                dirty = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts() {
        assert_eq!(choose_layout(80, 24), Some(WIDE));
        assert_eq!(choose_layout(46, 30), Some(COMPACT));
        assert_eq!(choose_layout(30, 30), None);
        assert!(WIDE.board_w() + 21 <= WIDE.width);
        assert!(COMPACT.board_w() <= COMPACT.width);
        assert_eq!(choose_layout(80, 23), None);
        assert_eq!(choose_layout(71, 24), Some(WIDE));
        assert_eq!(choose_layout(70, 25), Some(COMPACT));
        assert_eq!(choose_layout(46, 25), Some(COMPACT));
        assert_eq!(choose_layout(45, 24), None);
        assert_eq!(WIDE.board_h() + 1, WIDE.height);
        assert_eq!(COMPACT.board_h() + 2, COMPACT.height);
    }

    #[test]
    fn tiles_look_square() {
        // Terminal cells are about twice as tall as wide.
        for layout in [WIDE, COMPACT] {
            assert_eq!(layout.tile_w, 2 * layout.tile_h);
            assert_eq!(layout.gap_x, 2 * layout.gap_y);
        }
    }

    #[test]
    fn names_fit_on_a_compact_tile() {
        for tier in &TIERS {
            assert!(tier.name.len() <= COMPACT.tile_w, "{}", tier.name);
        }
    }
}
