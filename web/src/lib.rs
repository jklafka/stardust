//! WebAssembly exports for the web version. `web/www/app.js` is the other side
//! of this interface, so change the two together.
//!
//! These are plain exported functions rather than wasm-bindgen bindings, so
//! the page can load the module with no generated code or extra dependency.

use std::sync::Mutex;

use stardust_core::{Cell, Direction, Game, MAX_RANK, SIZE, Slide, THEMES};

struct Session {
    game: Game,
    slides: Vec<Slide>,
    spawned: Option<Cell>,
}

impl Session {
    fn new(seed: u32, best: u32) -> Self {
        Session {
            game: Game::new(seed as u64, best),
            slides: Vec::new(),
            spawned: None,
        }
    }

    /// Replaces the running game with a saved one. Returns false and changes
    /// nothing if the saved state is impossible. The random generator keeps
    /// its fresh seed. The best score is raised to at least the saved score.
    fn restore(
        &mut self,
        saved: &Staging,
        score: u32,
        won: bool,
        continued: bool,
        moves: u32,
    ) -> bool {
        let all_set = saved.filled == u16::MAX;
        let empty = saved.cells.iter().all(|&rank| rank == 0);
        // Scoring, winning and continuing all take at least one move.
        let no_moves_yet = moves == 0 && (score > 0 || won);
        if saved.bad || !all_set || empty || (continued && !won) || no_moves_yet {
            return false;
        }
        for (i, &rank) in saved.cells.iter().enumerate() {
            self.game.board[i / SIZE][i % SIZE] = rank;
        }
        self.game.score = score;
        self.game.best = self.game.best.max(score);
        self.game.won = won;
        self.game.continued = continued;
        self.game.moves = moves;
        self.slides.clear();
        self.spawned = None;
        true
    }

    fn try_move(&mut self, direction: Direction) -> bool {
        self.slides.clear();
        self.spawned = None;
        if self.game.showing_win() || self.game.over() {
            return false;
        }
        let Some((result, spawned)) = self.game.try_move(direction) else {
            return false;
        };
        self.slides = result.slides;
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

/// A saved board being loaded cell by cell, checked before it is applied.
struct Staging {
    cells: [u8; SIZE * SIZE],
    /// One bit per cell that has been loaded.
    filled: u16,
    /// A cell was out of range, so the whole load is rejected.
    bad: bool,
}

impl Staging {
    const fn new() -> Self {
        Staging {
            cells: [0; SIZE * SIZE],
            filled: 0,
            bad: false,
        }
    }

    fn set(&mut self, index: u32, rank: u32) -> bool {
        if index as usize >= SIZE * SIZE || rank > MAX_RANK as u32 {
            self.bad = true;
            return false;
        }
        self.cells[index as usize] = rank as u8;
        self.filled |= 1 << index;
        true
    }
}

static STAGING: Mutex<Staging> = Mutex::new(Staging::new());

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

/// Starts loading a saved game. Follow with `stardust_restore_cell` for each of
/// the 16 cells (index is row * 4 + column), then `stardust_restore_commit`.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_restore_begin() {
    *STAGING.lock().unwrap() = Staging::new();
}

/// Stages one cell. Returns 0 if the index or rank is out of range.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_restore_cell(index: u32, rank: u32) -> u32 {
    STAGING.lock().unwrap().set(index, rank) as u32
}

/// Applies the staged game if it is valid and returns 1. Otherwise returns 0
/// and the running game is untouched. `won` and `continued` are 0 or 1, and
/// `moves` is how many moves the game has had.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_restore_commit(score: u32, won: u32, continued: u32, moves: u32) -> u32 {
    let staged = std::mem::replace(&mut *STAGING.lock().unwrap(), Staging::new());
    if won > 1 || continued > 1 {
        return 0;
    }
    with_session(|session| session.restore(&staged, score, won == 1, continued == 1, moves) as u32)
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

/// Whether the top tile has been made, even if the message was dismissed.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_won() -> u32 {
    with_session(|session| session.game.won as u32)
}

/// Whether the player dismissed the win message and kept playing.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_continued() -> u32 {
    with_session(|session| session.game.continued as u32)
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

/// The cell of the tile the last move spawned (see `pack_cell`), or -1 if none.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_spawned() -> i32 {
    with_session(|session| session.spawned.map_or(-1, |cell| pack_cell(cell) as i32))
}

/// How many moves the current game has had.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_moves() -> u32 {
    with_session(|session| session.game.moves)
}

/// How many themes there are.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_theme_count() -> u32 {
    THEMES.len() as u32
}

/// The current theme's index.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_theme() -> u32 {
    with_session(|session| session.game.theme as u32)
}

/// Whether the theme may be changed now: before the first move, when the game
/// is over, or while the win message shows.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_can_switch_theme() -> u32 {
    with_session(|session| session.game.can_switch_theme() as u32)
}

/// Chooses a theme by index. Returns 1 if it is now the current theme, and 0
/// if the index is out of range or switching isn't allowed (unless it is
/// already current). The board and score never change.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_set_theme(index: u32) -> u32 {
    with_session(|session| {
        let index = index as usize;
        if index >= THEMES.len() {
            return 0;
        }
        if index != session.game.theme {
            if !session.game.can_switch_theme() {
                return 0;
            }
            session.game.set_theme(index);
        }
        1
    })
}

/// Where a theme's UTF-8 id is stored in the module's memory.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_theme_id_ptr(theme: u32) -> *const u8 {
    THEMES
        .get(theme as usize)
        .map_or(std::ptr::null(), |t| t.id.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn stardust_theme_id_len(theme: u32) -> u32 {
    THEMES.get(theme as usize).map_or(0, |t| t.id.len() as u32)
}

/// Where a theme's UTF-8 display name is stored.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_theme_name_ptr(theme: u32) -> *const u8 {
    THEMES
        .get(theme as usize)
        .map_or(std::ptr::null(), |t| t.name.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn stardust_theme_name_len(theme: u32) -> u32 {
    THEMES
        .get(theme as usize)
        .map_or(0, |t| t.name.len() as u32)
}

/// Where a theme's UTF-8 win message is stored.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_theme_win_ptr(theme: u32) -> *const u8 {
    THEMES
        .get(theme as usize)
        .map_or(std::ptr::null(), |t| t.win_message.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn stardust_theme_win_len(theme: u32) -> u32 {
    THEMES
        .get(theme as usize)
        .map_or(0, |t| t.win_message.len() as u32)
}

/// How many tiers each theme has.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_tier_count() -> u32 {
    MAX_RANK as u32
}

fn tier(theme: u32, tier: u32) -> Option<&'static stardust_core::Tier> {
    THEMES.get(theme as usize)?.tiers.get(tier as usize)
}

/// The glyph of a tier as a Unicode code point. Tier 0 has rank 1.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_tier_glyph(theme: u32, tier_index: u32) -> u32 {
    tier(theme, tier_index).map_or(0, |t| t.glyph as u32)
}

/// Where a tier's UTF-8 name is stored in the module's memory.
#[unsafe(no_mangle)]
pub extern "C" fn stardust_tier_name_ptr(theme: u32, tier_index: u32) -> *const u8 {
    tier(theme, tier_index).map_or(std::ptr::null(), |t| t.name.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn stardust_tier_name_len(theme: u32, tier_index: u32) -> u32 {
    tier(theme, tier_index).map_or(0, |t| t.name.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exports share one global session, so tests that use them take turns.
    static GLOBAL: Mutex<()> = Mutex::new(());

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
        assert!(session.spawned.is_some());
    }

    #[test]
    fn blocked_move_clears_the_last_move() {
        let mut session = session([[1, 1, 0, 0], [0; 4], [0; 4], [0; 4]]);
        session.try_move(Direction::Left);
        session.game.board = [[1, 2, 3, 4], [0; 4], [0; 4], [0; 4]];
        assert!(!session.try_move(Direction::Left));
        assert!(session.slides.is_empty());
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

    fn staged(cells: [u32; 16]) -> Staging {
        let mut staging = Staging::new();
        for (i, rank) in cells.into_iter().enumerate() {
            staging.set(i as u32, rank);
        }
        staging
    }

    fn valid_cells() -> [u32; 16] {
        let mut cells = [0; 16];
        cells[0] = 3;
        cells[5] = 11;
        cells[15] = 1;
        cells
    }

    #[test]
    fn restore_applies_valid_state() {
        let mut session = Session::new(1, 10);
        assert!(session.restore(&staged(valid_cells()), 40, true, true, 9));
        assert_eq!(session.game.board[0][0], 3);
        assert_eq!(session.game.board[1][1], 11);
        assert_eq!(session.game.board[3][3], 1);
        assert_eq!(session.game.score, 40);
        assert_eq!(session.game.best, 40);
        assert!(session.game.won && session.game.continued);
        assert!(!session.game.showing_win());
    }

    #[test]
    fn restore_keeps_a_higher_best() {
        let mut session = Session::new(1, 500);
        assert!(session.restore(&staged(valid_cells()), 40, false, false, 9));
        assert_eq!(session.game.best, 500);
    }

    #[test]
    fn restored_win_and_finished_game_show_their_messages() {
        let mut session = Session::new(1, 0);
        assert!(session.restore(&staged(valid_cells()), 8, true, false, 3));
        assert!(session.game.showing_win());
        let full = [1, 2, 1, 2, 2, 1, 2, 1, 1, 2, 1, 2, 2, 1, 2, 1];
        assert!(session.restore(&staged(full), 8, false, false, 3));
        assert!(session.game.over());
        assert!(!session.try_move(Direction::Left));
    }

    #[test]
    fn restore_rejects_invalid_state_and_leaves_the_game_alone() {
        let mut session = Session::new(1, 7);
        session.game.score = 12;
        let board = session.game.board;

        let mut bad_rank = valid_cells();
        bad_rank[2] = 12;
        let mut rank_99 = Staging::new();
        assert!(!rank_99.set(0, 99));
        let mut bad_index = staged(valid_cells());
        assert!(!bad_index.set(16, 1));
        let mut partial = Staging::new();
        for i in 0..15 {
            partial.set(i, 1);
        }

        let rejected = [
            staged(bad_rank),
            rank_99,
            bad_index,
            partial,
            staged([0; 16]),
        ];
        for saved in &rejected {
            assert!(!session.restore(saved, 5, false, false, 1));
        }
        // Scoring, winning or continuing without a move is impossible.
        assert!(!session.restore(&staged(valid_cells()), 5, false, false, 0));
        assert!(!session.restore(&staged(valid_cells()), 0, true, false, 0));
        // Continuing without having won is impossible.
        assert!(!session.restore(&staged(valid_cells()), 5, false, true, 1));

        assert_eq!(session.game.board, board);
        assert_eq!(session.game.score, 12);
        assert_eq!(session.game.best, 7);
        assert!(!session.game.won && !session.game.continued);
    }

    #[test]
    fn restore_exports_validate_and_reset_between_loads() {
        let _turn = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
        stardust_init(1, 0);
        stardust_restore_begin();
        assert_eq!(stardust_restore_cell(0, 99), 0);
        for i in 1..16 {
            stardust_restore_cell(i, 1);
        }
        assert_eq!(stardust_restore_commit(4, 0, 0, 2), 0);

        stardust_restore_begin();
        for i in 0..16 {
            assert_eq!(stardust_restore_cell(i, (i % 3) + 1), 1);
        }
        assert_eq!(stardust_restore_commit(4, 2, 0, 2), 0);
        assert_eq!(stardust_restore_commit(4, 0, 0, 2), 0); // staging was consumed
        stardust_restore_begin();
        for i in 0..16 {
            stardust_restore_cell(i, (i % 3) + 1);
        }
        assert_eq!(stardust_restore_commit(4, 0, 0, 2), 1);
        assert_eq!(stardust_score(), 4);
        assert_eq!(stardust_best(), 4);
        assert_eq!(stardust_cell(0, 1), 2);
    }

    #[test]
    fn restore_sets_moves_and_zero_moves_allows_themes() {
        let mut session = Session::new(1, 0);
        assert!(session.restore(&staged(valid_cells()), 0, false, false, 0));
        assert_eq!(session.game.moves, 0);
        assert!(session.game.can_switch_theme());
        assert!(session.restore(&staged(valid_cells()), 0, false, false, 4));
        assert_eq!(session.game.moves, 4);
        assert!(!session.game.can_switch_theme());
        assert!(session.restore(&staged(valid_cells()), 40, false, false, 1));
        assert!(!session.game.can_switch_theme());
    }

    #[test]
    fn theme_exports() {
        let _turn = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
        stardust_init(1, 0);
        assert_eq!(stardust_theme_count(), 2);
        assert_eq!(stardust_theme(), 0);
        assert_eq!(stardust_can_switch_theme(), 1);
        assert_eq!(stardust_set_theme(2), 0);
        assert_eq!(stardust_set_theme(1), 1);
        assert_eq!(stardust_theme(), 1);
        assert_eq!(stardust_theme_id_len(1), 6);
        assert_eq!(stardust_theme_name_len(0), 5);
        assert_eq!(stardust_theme_win_len(1), "You grew an Oak!".len() as u32);
        assert!(stardust_theme_id_ptr(2).is_null());
        assert!(stardust_theme_name_ptr(2).is_null());
        assert!(stardust_theme_win_ptr(2).is_null());
        assert_eq!(stardust_theme_win_len(2), 0);

        // Mid-game switching is refused, and the board is untouched.
        with_session(|session| {
            session.game.board = [[1, 1, 0, 0], [0; 4], [0; 4], [0; 4]];
        });
        assert_eq!(stardust_try_move(2), 1);
        assert_eq!(stardust_moves(), 1);
        assert_eq!(stardust_can_switch_theme(), 0);
        let (score, cell) = (stardust_score(), stardust_cell(0, 0));
        assert_eq!(stardust_set_theme(0), 0);
        assert_eq!(stardust_set_theme(1), 1); // already current
        assert_eq!(stardust_theme(), 1);
        assert_eq!((stardust_score(), stardust_cell(0, 0)), (score, cell));
        stardust_new_game();
        assert_eq!(stardust_moves(), 0);
        assert_eq!(stardust_theme(), 1); // survives a new game
        assert_eq!(stardust_set_theme(0), 1);
    }

    #[test]
    fn tier_exports_describe_every_tier() {
        assert_eq!(stardust_tier_count(), 11);
        assert_eq!(stardust_tier_name_len(0, 0), 4);
        assert_eq!(stardust_tier_glyph(0, 10), '🌌' as u32);
        assert_eq!(stardust_tier_glyph(1, 10), '🌳' as u32);
        assert_eq!(stardust_tier_name_len(1, 5), 9);
        assert_eq!(stardust_tier_glyph(0, 11), 0);
        assert_eq!(stardust_tier_glyph(2, 0), 0);
        assert_eq!(stardust_tier_name_len(0, 11), 0);
        assert!(stardust_tier_name_ptr(0, 11).is_null());
        assert!(stardust_tier_name_ptr(2, 0).is_null());
    }
}
