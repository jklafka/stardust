import random
import unicodedata
import unittest

from stardust import (
    COMPACT,
    MAX_RANK,
    TIERS,
    WIDE,
    Game,
    Screen,
    choose_layout,
    has_moves,
    parse_input,
    slide,
    swipe_direction,
    tile_value,
)

T = MAX_RANK


def row(*ranks):
    return [list(ranks), [0] * 4, [0] * 4, [0] * 4]


class Tiers(unittest.TestCase):
    def test_eleven_tiers_with_unique_names_and_glyphs(self):
        self.assertEqual(len(TIERS), 11)
        self.assertEqual(len({t.name for t in TIERS}), 11)
        self.assertEqual(len({t.glyph for t in TIERS}), 11)

    def test_values_double_up_the_hierarchy(self):
        self.assertEqual([tile_value(r) for r in range(1, 12)], [2 ** i for i in range(11)])

    def test_glyphs_are_one_double_width_codepoint(self):
        for tier in TIERS:
            self.assertEqual(len(tier.glyph), 1, tier.name)
            self.assertEqual(unicodedata.east_asian_width(tier.glyph), "W", tier.name)

    def test_names_fit_on_a_compact_tile(self):
        for tier in TIERS:
            self.assertLessEqual(len(tier.name), COMPACT.tile_w)


class Slide(unittest.TestCase):
    def cases(self, line, direction="left"):
        return slide(row(*line), direction).board[0]

    def test_slides_to_wall(self):
        self.assertEqual(self.cases((0, 0, 3, 0)), [3, 0, 0, 0])

    def test_merges_same_tiles(self):
        self.assertEqual(self.cases((1, 1, 0, 0)), [2, 0, 0, 0])

    def test_merges_across_a_gap(self):
        self.assertEqual(self.cases((1, 0, 0, 1)), [2, 0, 0, 0])

    def test_different_tiles_do_not_merge(self):
        self.assertEqual(self.cases((1, 2, 1, 2)), [1, 2, 1, 2])

    def test_tile_merges_only_once_per_move(self):
        self.assertEqual(self.cases((1, 1, 2, 0)), [2, 2, 0, 0])
        self.assertEqual(self.cases((2, 1, 1, 0)), [2, 2, 0, 0])

    def test_four_of_a_kind_make_two_pairs(self):
        self.assertEqual(self.cases((1, 1, 1, 1)), [2, 2, 0, 0])

    def test_three_merge_nearest_the_wall(self):
        self.assertEqual(self.cases((1, 1, 1, 0)), [2, 1, 0, 0])
        self.assertEqual(self.cases((1, 1, 1, 0), "right"), [0, 0, 1, 2])

    def test_top_tiles_do_not_merge(self):
        self.assertEqual(self.cases((T, T, 0, 0)), [T, T, 0, 0])

    def test_making_the_top_tile(self):
        self.assertEqual(self.cases((T - 1, T - 1, 0, 0)), [T, 0, 0, 0])

    def test_all_directions(self):
        board = [[0, 0, 0, 0], [0, 1, 0, 0], [0, 1, 0, 0], [0, 0, 0, 0]]
        self.assertEqual(slide(board, "up").board[0][1], 2)
        self.assertEqual(slide(board, "down").board[3][1], 2)
        horizontal = slide(board, "left").board
        self.assertEqual((horizontal[1][0], horizontal[2][0]), (1, 1))
        horizontal = slide(board, "right").board
        self.assertEqual((horizontal[1][3], horizontal[2][3]), (1, 1))

    def test_score_is_value_of_created_tile(self):
        self.assertEqual(slide(row(1, 1, 3, 3), "left").gained, 2 + 8)

    def test_blocked_move_is_unchanged(self):
        self.assertFalse(slide(row(1, 2, 3, 4), "left").changed)
        self.assertFalse(slide(row(T, T, 0, 0), "left").changed)


FULL = [[1, 2, 1, 2], [2, 1, 2, 1], [1, 2, 1, 2], [2, 1, 2, 1]]


class EndOfGame(unittest.TestCase):
    def test_empty_cell_means_moves(self):
        board = [r[:] for r in FULL]
        board[2][2] = 0
        self.assertTrue(has_moves(board))

    def test_full_board_without_pairs_is_over(self):
        self.assertFalse(has_moves(FULL))

    def test_horizontal_and_vertical_pairs_keep_playing(self):
        board = [r[:] for r in FULL]
        board[3][3] = board[3][2]
        self.assertTrue(has_moves(board))
        board = [r[:] for r in FULL]
        board[3][3] = board[2][3]
        self.assertTrue(has_moves(board))

    def test_diagonal_pairs_do_not_count(self):
        self.assertFalse(has_moves(FULL))  # Every diagonal in FULL matches.

    def test_adjacent_top_tiles_do_not_count(self):
        board = [r[:] for r in FULL]
        board[0][0] = board[0][1] = T
        board[1][0], board[1][1] = 3, 4
        self.assertFalse(has_moves(board))


class GameFlow(unittest.TestCase):
    def game(self):
        return Game(random.Random(1))

    def test_new_game_starts_with_two_tiles(self):
        g = self.game()
        self.assertEqual(sum(1 for r in g.board for v in r if v), 2)

    def test_spawns_only_the_two_lowest_tiers_mostly_dust(self):
        g = self.game()
        seen = []
        for _ in range(2000):
            g.board = [[0] * 4 for _ in range(4)]
            r, c = g.spawn()
            seen.append(g.board[r][c])
        self.assertEqual(set(seen), {1, 2})
        self.assertGreater(seen.count(1), seen.count(2) * 5)

    def test_move_spawns_one_tile_and_scores(self):
        g = self.game()
        g.board = row(1, 1, 0, 0)
        result, spawned = g.move("left")
        self.assertIsNotNone(spawned)
        self.assertEqual(g.score, 2)
        self.assertEqual(g.best, 2)
        self.assertEqual(sum(1 for r in g.board for v in r if v), 2)

    def test_blocked_move_spawns_nothing(self):
        g = self.game()
        g.board = row(1, 2, 3, 4)
        self.assertIsNone(g.move("left"))
        self.assertEqual(sum(1 for r in g.board for v in r if v), 4)

    def test_win_then_continue(self):
        g = self.game()
        g.board = row(T - 1, T - 1, 0, 0)
        g.move("left")
        self.assertTrue(g.showing_win)
        g.continued = True
        self.assertFalse(g.showing_win)
        g.board = row(T, 1, 1, 0)
        g.move("left")
        self.assertFalse(g.showing_win)

    def test_new_game_keeps_best_and_resets_the_rest(self):
        g = self.game()
        g.board = row(T - 1, T - 1, 0, 0)
        g.move("left")
        best = g.best
        g.new()
        self.assertEqual((g.score, g.best, g.won), (0, best, False))

    def test_random_games_preserve_invariants_and_end(self):
        for seed in range(50):
            g = Game(random.Random(seed))
            rng = random.Random(seed)
            for _ in range(5000):
                if g.over:
                    break
                total = sum(tile_value(v) for r in g.board for v in r if v)
                moved = g.move(rng.choice(["up", "down", "left", "right"]))
                if moved:
                    result, spawned = moved
                    spawn_rank = g.board[spawned[0]][spawned[1]]
                    self.assertIn(spawn_rank, (1, 2))
                    after = sum(tile_value(v) for r in g.board for v in r if v)
                    # Merging conserves total value, so only the spawn adds to it.
                    self.assertEqual(after, total + tile_value(spawn_rank))
            self.assertTrue(g.over, f"seed {seed} did not finish")
            self.assertTrue(all(all(r) for r in g.board))


class Input(unittest.TestCase):
    def test_arrows_in_both_cursor_modes(self):
        self.assertEqual(parse_input(b"\x1b[A\x1b[B\x1b[C\x1b[D")[0], ["up", "down", "right", "left"])
        self.assertEqual(parse_input(b"\x1bOA\x1bOD")[0], ["up", "left"])

    def test_keys(self):
        self.assertEqual(parse_input(b"qnc\x03x")[0], ["quit", "new", "continue", "quit"])

    def test_split_sequence_is_kept(self):
        events, rest = parse_input(b"\x1b[")
        self.assertEqual((events, rest), ([], b"\x1b["))
        self.assertEqual(parse_input(rest + b"A"), (["up"], b""))

    def test_split_mouse_sequence_is_kept(self):
        events, rest = parse_input(b"\x1b[<0;12")
        self.assertEqual((events, rest), ([], b"\x1b[<0;12"))
        self.assertEqual(parse_input(rest + b";5M")[0], [("press", 12, 5)])

    def test_mouse_press_and_release(self):
        self.assertEqual(
            parse_input(b"\x1b[<0;10;5M\x1b[<0;20;5m")[0],
            [("press", 10, 5), ("release", 20, 5)],
        )

    def test_wheel_is_ignored(self):
        self.assertEqual(parse_input(b"\x1b[<64;10;5M\x1b[<65;10;5M")[0], [])

    def test_unknown_sequences_do_not_trigger_keys(self):
        self.assertEqual(parse_input(b"\x1b[200~")[0], [])

    def test_swipe(self):
        self.assertEqual(swipe_direction(10, 10, 20, 10), "right")
        self.assertEqual(swipe_direction(20, 10, 10, 10), "left")
        self.assertEqual(swipe_direction(10, 10, 10, 15), "down")
        self.assertEqual(swipe_direction(10, 15, 10, 10), "up")

    def test_tap_is_not_a_swipe(self):
        self.assertIsNone(swipe_direction(10, 10, 10, 10))
        self.assertIsNone(swipe_direction(10, 10, 11, 10))


class Rendering(unittest.TestCase):
    def test_layouts(self):
        self.assertIs(choose_layout(80, 24), WIDE)
        self.assertIs(choose_layout(45, 30), COMPACT)
        self.assertIsNone(choose_layout(30, 30))
        self.assertLessEqual(WIDE.board_w + 21, WIDE.width)
        self.assertLessEqual(COMPACT.board_w, COMPACT.width)

    def test_overwriting_half_a_wide_glyph_clears_the_other_half(self):
        s = Screen(6, 1)
        s.put(1, 0, "✨")
        s.put(2, 0, "x")
        self.assertEqual(s.cells[0][1][0], " ")
        s = Screen(6, 1)
        s.put(1, 0, "✨")
        s.put(1, 0, "x")
        self.assertEqual(s.cells[0][2][0], " ")

    def test_unchanged_rows_are_not_redrawn(self):
        s = Screen(4, 2)
        s.put(0, 0, "a")
        self.assertIn("a", s.render())
        self.assertEqual(s.render(), "")
        s.put(0, 1, "b")
        output = s.render()
        self.assertIn("b", output)
        self.assertNotIn("a", output)


if __name__ == "__main__":
    unittest.main()
