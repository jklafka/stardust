//! The rules of the game, independent of the terminal.

pub const SIZE: usize = 4;

pub struct Tier {
    pub name: &'static str,
    pub glyph: char,
    /// xterm-256 background.
    pub bg: u8,
    /// xterm-256 foreground.
    pub fg: u8,
}

const fn tier(name: &'static str, glyph: char, bg: u8, fg: u8) -> Tier {
    Tier {
        name,
        glyph,
        bg,
        fg,
    }
}

/// Index + 1 is the tile's rank. Two tiles of rank n merge into rank n + 1.
pub const TIERS: [Tier; 11] = [
    tier("Dust", '✨', 245, 16),
    tier("Rock", '🪨', 94, 230),
    tier("Comet", '🌠', 31, 231),
    tier("Moon", '🌕', 250, 16),
    tier("Planet", '🌍', 153, 16),
    tier("Gas Giant", '🪐', 61, 231),
    tier("Star", '⭐', 202, 16),
    tier("Red Giant", '🔴', 52, 231),
    tier("Black Hole", '⚫', 134, 231),
    tier("Galaxy", '🌀', 213, 16),
    tier("Universe", '🌌', 231, 16),
];
pub const MAX_RANK: u8 = TIERS.len() as u8;

pub fn tier_of(rank: u8) -> &'static Tier {
    &TIERS[rank as usize - 1]
}

/// Tile ranks, with 0 for an empty cell.
pub type Board = [[u8; SIZE]; SIZE];
/// (row, column)
pub type Cell = (usize, usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Points for creating a tile: 1, 2, 4, ... 1024.
pub fn tile_value(rank: u8) -> u32 {
    1 << (rank - 1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slide {
    pub from: Cell,
    pub to: Cell,
    pub rank: u8,
}

#[derive(Debug)]
pub struct MoveResult {
    /// The board after the move, before a new tile spawns.
    pub board: Board,
    /// Every tile that was on the board before the move.
    pub slides: Vec<Slide>,
    pub merged: Vec<Cell>,
    pub gained: u32,
}

impl MoveResult {
    pub fn changed(&self) -> bool {
        self.slides.iter().any(|slide| slide.from != slide.to)
    }
}

/// Applies a move. Each tile merges at most once; top-rank tiles never merge.
pub fn slide(board: &Board, direction: Direction) -> MoveResult {
    let mut result = MoveResult {
        board: [[0; SIZE]; SIZE],
        slides: Vec::new(),
        merged: Vec::new(),
        gained: 0,
    };
    for line in 0..SIZE {
        // Cells ordered from the wall the tiles are moving toward.
        let cell_at = |i: usize| match direction {
            Direction::Up => (i, line),
            Direction::Down => (SIZE - 1 - i, line),
            Direction::Left => (line, i),
            Direction::Right => (line, SIZE - 1 - i),
        };
        let mut target = 0;
        let mut mergeable = false; // Can the tile just placed still accept a merge?
        for i in 0..SIZE {
            let from = cell_at(i);
            let rank = board[from.0][from.1];
            if rank == 0 {
                continue;
            }
            let previous = (target > 0).then(|| cell_at(target - 1));
            let to = match previous {
                Some(to) if mergeable && rank < MAX_RANK && result.board[to.0][to.1] == rank => {
                    result.board[to.0][to.1] = rank + 1;
                    result.merged.push(to);
                    result.gained += tile_value(rank + 1);
                    mergeable = false;
                    to
                }
                _ => {
                    let to = cell_at(target);
                    result.board[to.0][to.1] = rank;
                    target += 1;
                    mergeable = true;
                    to
                }
            };
            result.slides.push(Slide { from, to, rank });
        }
    }
    result
}

/// False only when the grid is full and no neighbours can combine.
pub fn has_moves(board: &Board) -> bool {
    for r in 0..SIZE {
        for c in 0..SIZE {
            let rank = board[r][c];
            if rank == 0 {
                return true;
            }
            if rank < MAX_RANK
                && ((c + 1 < SIZE && board[r][c + 1] == rank)
                    || (r + 1 < SIZE && board[r + 1][c] == rank))
            {
                return true;
            }
        }
    }
    false
}

/// SplitMix64. Tile placement needs variety, not cryptographic strength.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }
}

pub struct Game {
    pub board: Board,
    pub score: u32,
    pub best: u32,
    /// The top tile has been made.
    pub won: bool,
    /// The player acknowledged the win message and kept playing.
    pub continued: bool,
    rng: Rng,
}

impl Game {
    pub fn new(seed: u64, best: u32) -> Self {
        let mut game = Game {
            board: [[0; SIZE]; SIZE],
            score: 0,
            best,
            won: false,
            continued: false,
            rng: Rng(seed),
        };
        game.reset();
        game
    }

    /// Starts a new game, keeping the best score.
    pub fn reset(&mut self) {
        self.board = [[0; SIZE]; SIZE];
        self.score = 0;
        self.won = false;
        self.continued = false;
        self.spawn();
        self.spawn();
    }

    pub fn spawn(&mut self) -> Option<Cell> {
        let mut empty = Vec::with_capacity(SIZE * SIZE);
        for r in 0..SIZE {
            for c in 0..SIZE {
                if self.board[r][c] == 0 {
                    empty.push((r, c));
                }
            }
        }
        if empty.is_empty() {
            return None;
        }
        let (r, c) = empty[self.rng.below(empty.len())];
        self.board[r][c] = if self.rng.below(10) == 0 { 2 } else { 1 };
        Some((r, c))
    }

    pub fn over(&self) -> bool {
        !has_moves(&self.board)
    }

    pub fn highest(&self) -> u8 {
        self.board.iter().flatten().copied().max().unwrap_or(0)
    }

    pub fn showing_win(&self) -> bool {
        self.won && !self.continued
    }

    /// Returns the move and the spawned cell, or None if nothing could move.
    pub fn try_move(&mut self, direction: Direction) -> Option<(MoveResult, Option<Cell>)> {
        let result = slide(&self.board, direction);
        if !result.changed() {
            return None;
        }
        self.board = result.board;
        self.score += result.gained;
        self.best = self.best.max(self.score);
        if !self.won && self.highest() == MAX_RANK {
            self.won = true;
        }
        let spawned = self.spawn();
        Some((result, spawned))
    }
}

#[cfg(test)]
mod tests {
    use super::Direction::*;
    use super::*;

    const T: u8 = MAX_RANK;
    const FULL: Board = [[1, 2, 1, 2], [2, 1, 2, 1], [1, 2, 1, 2], [2, 1, 2, 1]];

    fn row(line: [u8; 4]) -> Board {
        [line, [0; 4], [0; 4], [0; 4]]
    }

    fn slid(line: [u8; 4], direction: Direction) -> [u8; 4] {
        slide(&row(line), direction).board[0]
    }

    fn count(board: &Board) -> usize {
        board.iter().flatten().filter(|&&rank| rank != 0).count()
    }

    fn total(board: &Board) -> u32 {
        board
            .iter()
            .flatten()
            .filter(|&&rank| rank != 0)
            .map(|&rank| tile_value(rank))
            .sum()
    }

    #[test]
    fn eleven_tiers_with_unique_names_and_glyphs() {
        let names: std::collections::HashSet<_> = TIERS.iter().map(|t| t.name).collect();
        let glyphs: std::collections::HashSet<_> = TIERS.iter().map(|t| t.glyph).collect();
        assert_eq!((TIERS.len(), names.len(), glyphs.len()), (11, 11, 11));
    }

    #[test]
    fn values_double_up_the_hierarchy() {
        for rank in 1..=MAX_RANK {
            assert_eq!(tile_value(rank), 2u32.pow(rank as u32 - 1));
        }
    }

    #[test]
    fn slides_to_wall() {
        assert_eq!(slid([0, 0, 3, 0], Left), [3, 0, 0, 0]);
    }

    #[test]
    fn merges_same_tiles() {
        assert_eq!(slid([1, 1, 0, 0], Left), [2, 0, 0, 0]);
    }

    #[test]
    fn merges_across_a_gap() {
        assert_eq!(slid([1, 0, 0, 1], Left), [2, 0, 0, 0]);
    }

    #[test]
    fn different_tiles_do_not_merge() {
        assert_eq!(slid([1, 2, 1, 2], Left), [1, 2, 1, 2]);
    }

    #[test]
    fn tile_merges_only_once_per_move() {
        assert_eq!(slid([1, 1, 2, 0], Left), [2, 2, 0, 0]);
        assert_eq!(slid([2, 1, 1, 0], Left), [2, 2, 0, 0]);
    }

    #[test]
    fn four_of_a_kind_make_two_pairs() {
        assert_eq!(slid([1, 1, 1, 1], Left), [2, 2, 0, 0]);
    }

    #[test]
    fn three_merge_nearest_the_wall() {
        assert_eq!(slid([1, 1, 1, 0], Left), [2, 1, 0, 0]);
        assert_eq!(slid([1, 1, 1, 0], Right), [0, 0, 1, 2]);
    }

    #[test]
    fn top_tiles_do_not_merge() {
        assert_eq!(slid([T, T, 0, 0], Left), [T, T, 0, 0]);
    }

    #[test]
    fn making_the_top_tile() {
        assert_eq!(slid([T - 1, T - 1, 0, 0], Left), [T, 0, 0, 0]);
    }

    #[test]
    fn all_directions() {
        let board = [[0, 0, 0, 0], [0, 1, 0, 0], [0, 1, 0, 0], [0, 0, 0, 0]];
        assert_eq!(slide(&board, Up).board[0][1], 2);
        assert_eq!(slide(&board, Down).board[3][1], 2);
        let left = slide(&board, Left).board;
        assert_eq!((left[1][0], left[2][0]), (1, 1));
        let right = slide(&board, Right).board;
        assert_eq!((right[1][3], right[2][3]), (1, 1));
    }

    #[test]
    fn score_is_value_of_created_tile() {
        assert_eq!(slide(&row([1, 1, 3, 3]), Left).gained, 2 + 8);
    }

    #[test]
    fn blocked_move_is_unchanged() {
        assert!(!slide(&row([1, 2, 3, 4]), Left).changed());
        assert!(!slide(&row([T, T, 0, 0]), Left).changed());
    }

    #[test]
    fn empty_cell_means_moves() {
        let mut board = FULL;
        board[2][2] = 0;
        assert!(has_moves(&board));
    }

    #[test]
    fn full_board_without_pairs_is_over() {
        // Every diagonal in FULL matches, so diagonal pairs don't count either.
        assert!(!has_moves(&FULL));
    }

    #[test]
    fn horizontal_and_vertical_pairs_keep_playing() {
        let mut board = FULL;
        board[3][3] = board[3][2];
        assert!(has_moves(&board));
        let mut board = FULL;
        board[3][3] = board[2][3];
        assert!(has_moves(&board));
    }

    #[test]
    fn adjacent_top_tiles_do_not_count_as_a_move() {
        let mut board = FULL;
        board[0][0] = T;
        board[0][1] = T;
        board[1][0] = 3;
        board[1][1] = 4;
        assert!(!has_moves(&board));
    }

    #[test]
    fn new_game_starts_with_two_tiles() {
        assert_eq!(count(&Game::new(1, 0).board), 2);
    }

    #[test]
    fn spawns_only_the_two_lowest_tiers_mostly_dust() {
        let mut game = Game::new(1, 0);
        let (mut dust, mut rock) = (0, 0);
        for _ in 0..2000 {
            game.board = [[0; SIZE]; SIZE];
            let (r, c) = game.spawn().unwrap();
            match game.board[r][c] {
                1 => dust += 1,
                2 => rock += 1,
                other => panic!("spawned rank {other}"),
            }
        }
        assert!(rock > 0);
        assert!(dust > rock * 5);
    }

    #[test]
    fn move_spawns_one_tile_and_scores() {
        let mut game = Game::new(1, 0);
        game.board = row([1, 1, 0, 0]);
        let (_, spawned) = game.try_move(Left).unwrap();
        assert!(spawned.is_some());
        assert_eq!((game.score, game.best), (2, 2));
        assert_eq!(count(&game.board), 2);
    }

    #[test]
    fn blocked_move_spawns_nothing() {
        let mut game = Game::new(1, 0);
        game.board = row([1, 2, 3, 4]);
        assert!(game.try_move(Left).is_none());
        assert_eq!(count(&game.board), 4);
    }

    #[test]
    fn win_then_continue() {
        let mut game = Game::new(1, 0);
        game.board = row([T - 1, T - 1, 0, 0]);
        game.try_move(Left);
        assert!(game.showing_win());
        game.continued = true;
        assert!(!game.showing_win());
        game.board = row([T, 1, 1, 0]);
        game.try_move(Left);
        assert!(!game.showing_win());
    }

    #[test]
    fn new_game_keeps_best_and_resets_the_rest() {
        let mut game = Game::new(1, 0);
        game.board = row([T - 1, T - 1, 0, 0]);
        game.try_move(Left);
        let best = game.best;
        game.reset();
        assert_eq!((game.score, game.best, game.won), (0, best, false));
    }

    #[test]
    fn random_games_preserve_invariants_and_end() {
        for seed in 0..50 {
            let mut game = Game::new(seed, 0);
            let mut chooser = Rng(seed);
            let mut moves = 0;
            while !game.over() {
                moves += 1;
                assert!(moves < 100_000, "seed {seed} did not finish");
                let before = total(&game.board);
                let direction = [Up, Down, Left, Right][chooser.below(4)];
                if let Some((result, spawned)) = game.try_move(direction) {
                    // Merging conserves total value, so only the spawn adds to it.
                    assert_eq!(total(&result.board), before);
                    let (r, c) = spawned.unwrap();
                    assert!(matches!(game.board[r][c], 1 | 2));
                }
            }
            assert_eq!(count(&game.board), SIZE * SIZE);
        }
    }
}
