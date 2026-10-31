//! WebAssembly exports for the web version. `web/www/app.js` is the other side
//! of this interface, so change the two together.
//!
//! These are plain exported functions rather than wasm-bindgen bindings, so
//! the page can load the module with no generated code or extra dependency.

use std::sync::Mutex;

use stardust_core::{Cell, Direction, Game, SIZE, Slide, TIERS};

struct Session {
    game: Game,
    slides: Vec<Slide>,
    merged: Vec<Cell>,
    spawned: Option<Cell>,
}

impl Session {
    fn new(seed: u32, best: u32) -> Self {
        Session {
            game: Game::new(seed as u64, best),
            slides: Vec::new(),
            merged: Vec::new(),
            spawned: None,
        }
    }

    fn try_move(&mut self, direction: Direction) -> bool {
        self.slides.clear();
        self.merged.clear();
        self.spawned = None;
        if self.game.showing_win() || self.game.over() {
            return false;
        }
        let Some((result, spawned)) = self.game.try_move(direction) else {
            return false;
        };
        self.slides = result.slides;
        self.merged = result.merged;
        self.spawned = spawned;
        true
    }
}

fn direction(code: u32) -> Option<Direction> {
    match code {
        0 => Some(Direction::Up),
        1 => Some(Direction::Down),
        2 => Some(Direction::Left),
        3 => Some(Direction::Right),
        _ => None,
    }
}

/// Bits, lowest first: from row, from column, to row, to column (4 each), rank.
fn pack_slide(slide: &Slide) -> u32 {
    slide.from.0 as u32
        | (slide.from.1 as u32) << 4
        | (slide.to.0 as u32) << 8
        | (slide.to.1 as u32) << 12
        | (slide.rank as u32) << 16
}

/// Bits, lowest first: row, column (4 each).
fn pack_cell((r, c): Cell) -> u32 {
    r as u32 | (c as u32) << 4
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

/// Runs `action` on the session, or returns the default before `stardust_init`.
fn with_session<T: Default>(action: impl FnOnce(&mut Session) -> T) -> T {
    let mut session = SESSION.lock().unwrap();
    session.as_mut().map(action).unwrap_or_default()
}

/// Starts the first game. Must be called before anything else.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_init(seed: u32, best: u32) {
    *SESSION.lock().unwrap() = Some(Session::new(seed, best));
}

/// Starts a new game, keeping the best score.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_new_game() {
    with_session(|session| {
        session.game.reset();
        session.slides.clear();
        session.merged.clear();
        session.spawned = None;
    });
}

/// Directions: 0 up, 1 down, 2 left, 3 right. Returns 1 if the tiles moved.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_try_move(code: u32) -> u32 {
    with_session(|session| direction(code).is_some_and(|d| session.try_move(d)) as u32)
}

/// Dismisses the win message so play can continue.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_continue() {
    with_session(|session| {
        if session.game.showing_win() {
            session.game.continued = true;
        }
    });
}

/// The rank at a cell, or 0 if it's empty or off the board.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_cell(r: u32, c: u32) -> u32 {
    with_session(|session| {
        let (r, c) = (r as usize, c as usize);
        if r < SIZE && c < SIZE {
            session.game.board[r][c] as u32
        } else {
            0
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn stardust_score() -> u32 {
    with_session(|session| session.game.score)
}

#[unsafe(no_mangle)]
pub extern "C" fn stardust_best() -> u32 {
    with_session(|session| session.game.best)
}

#[unsafe(no_mangle)]
pub extern "C" fn stardust_showing_win() -> u32 {
    with_session(|session| session.game.showing_win() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn stardust_over() -> u32 {
    with_session(|session| session.game.over() as u32)
}

/// How many tiles the last move slid. See `pack_slide`.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_slide_count() -> u32 {
    with_session(|session| session.slides.len() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn stardust_slide(index: u32) -> u32 {
    with_session(|session| session.slides.get(index as usize).map_or(0, pack_slide))
}

/// How many merges the last move made. See `pack_cell`.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_merged_count() -> u32 {
    with_session(|session| session.merged.len() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn stardust_merged(index: u32) -> u32 {
    with_session(|session| {
        session
            .merged
            .get(index as usize)
            .copied()
            .map_or(0, pack_cell)
    })
}

/// The cell of the tile the last move spawned (see `pack_cell`), or -1 if none.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_spawned() -> i32 {
    with_session(|session| session.spawned.map_or(-1, |cell| pack_cell(cell) as i32))
}

#[unsafe(no_mangle)]
pub extern "C" fn stardust_tier_count() -> u32 {
    TIERS.len() as u32
}

/// The glyph of a tier as a Unicode code point. Tier 0 has rank 1.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_tier_glyph(tier: u32) -> u32 {
    TIERS.get(tier as usize).map_or(0, |t| t.glyph as u32)
}

/// Where a tier's UTF-8 name is stored in the module's memory.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_tier_name_ptr(tier: u32) -> *const u8 {
    TIERS
        .get(tier as usize)
        .map_or(std::ptr::null(), |t| t.name.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn stardust_tier_name_len(tier: u32) -> u32 {
    TIERS.get(tier as usize).map_or(0, |t| t.name.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(board: stardust_core::Board) -> Session {
        let mut session = Session::new(1, 0);
        session.game.board = board;
        session
    }

    #[test]
    fn slides_pack_into_their_fields() {
        let slide = Slide {
            from: (1, 2),
            to: (3, 0),
            rank: 11,
        };
        assert_eq!(pack_slide(&slide), 1 | 2 << 4 | 3 << 8 | 11 << 16);
        assert_eq!(pack_cell((3, 2)), 3 | 2 << 4);
    }

    #[test]
    fn direction_codes() {
        assert_eq!(direction(0), Some(Direction::Up));
        assert_eq!(direction(1), Some(Direction::Down));
        assert_eq!(direction(2), Some(Direction::Left));
        assert_eq!(direction(3), Some(Direction::Right));
        assert_eq!(direction(4), None);
    }

    #[test]
    fn move_records_what_the_page_animates() {
        let mut session = session([[1, 1, 0, 0], [0; 4], [0; 4], [0; 4]]);
        assert!(session.try_move(Direction::Left));
        assert_eq!(session.slides.len(), 2);
        assert_eq!(session.merged, [(0, 0)]);
        assert!(session.spawned.is_some());
    }

    #[test]
    fn blocked_move_clears_the_last_move() {
        let mut session = session([[1, 1, 0, 0], [0; 4], [0; 4], [0; 4]]);
        session.try_move(Direction::Left);
        session.game.board = [[1, 2, 3, 4], [0; 4], [0; 4], [0; 4]];
        assert!(!session.try_move(Direction::Left));
        assert!(session.slides.is_empty());
        assert!(session.merged.is_empty());
        assert_eq!(session.spawned, None);
    }

    #[test]
    fn win_message_blocks_moves_until_dismissed() {
        let mut session = session([[10, 10, 0, 0], [0; 4], [0; 4], [0; 4]]);
        assert!(session.try_move(Direction::Left));
        assert!(session.game.showing_win());
        assert!(!session.try_move(Direction::Right));
        session.game.continued = true;
        assert!(session.try_move(Direction::Right));
    }

    #[test]
    fn finished_game_refuses_moves() {
        let full = [[1, 2, 1, 2], [2, 1, 2, 1], [1, 2, 1, 2], [2, 1, 2, 1]];
        let mut session = session(full);
        assert!(session.game.over());
        assert!(!session.try_move(Direction::Left));
    }

    #[test]
    fn tier_exports_describe_every_tier() {
        assert_eq!(stardust_tier_count(), 11);
        assert_eq!(stardust_tier_name_len(0), 4);
        assert_eq!(stardust_tier_glyph(10), '🌌' as u32);
        assert_eq!(stardust_tier_glyph(11), 0);
        assert_eq!(stardust_tier_name_len(11), 0);
        assert!(stardust_tier_name_ptr(11).is_null());
    }
}
