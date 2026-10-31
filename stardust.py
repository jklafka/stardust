#!/usr/bin/env python3
"""Stardust: merge cosmic matter on a 4x4 grid, from Dust up to a Universe.

Run with: python3 stardust.py
"""
from __future__ import annotations

import argparse
import os
import random
import re
import select
import shutil
import signal
import sys
import termios
import time
import unicodedata
from dataclasses import dataclass, field
from pathlib import Path
from typing import List, Optional, Sequence, Tuple

# ---------------------------------------------------------------- game rules

SIZE = 4


@dataclass(frozen=True)
class Tier:
    name: str
    glyph: str
    bg: int  # xterm-256 background
    fg: int  # xterm-256 foreground


# Index + 1 is the tile's rank. Two tiles of rank n merge into rank n + 1.
TIERS = (
    Tier("Dust", "✨", 245, 16),
    Tier("Rock", "🪨", 94, 230),
    Tier("Comet", "🌠", 31, 231),
    Tier("Moon", "🌕", 250, 16),
    Tier("Planet", "🌍", 153, 16),
    Tier("Gas Giant", "🪐", 61, 231),
    Tier("Star", "⭐", 202, 16),
    Tier("Red Giant", "🔴", 52, 231),
    Tier("Black Hole", "⚫", 134, 231),
    Tier("Galaxy", "🌀", 213, 16),
    Tier("Universe", "🌌", 231, 16),
)
MAX_RANK = len(TIERS)

Board = List[List[int]]
Cell = Tuple[int, int]

DIRECTIONS = {"up": (-1, 0), "down": (1, 0), "left": (0, -1), "right": (0, 1)}


def tile_value(rank: int) -> int:
    """Points for creating a tile: 1, 2, 4, ... 1024."""
    return 2 ** (rank - 1)


@dataclass
class MoveResult:
    board: Board
    # Every tile on the board before the move: (from_cell, to_cell, rank).
    slides: List[Tuple[Cell, Cell, int]] = field(default_factory=list)
    merged: List[Cell] = field(default_factory=list)
    gained: int = 0

    @property
    def changed(self) -> bool:
        return any(src != dst for src, dst, _ in self.slides)


def slide(board: Sequence[Sequence[int]], direction: str) -> MoveResult:
    """Apply a move. Each tile merges at most once; top-rank tiles never merge."""
    dr, dc = DIRECTIONS[direction]
    result = MoveResult(board=[[0] * SIZE for _ in range(SIZE)])
    for line in range(SIZE):
        # Cells ordered from the wall the tiles are moving toward.
        if dr:
            order = [(i if dr < 0 else SIZE - 1 - i, line) for i in range(SIZE)]
        else:
            order = [(line, i if dc < 0 else SIZE - 1 - i) for i in range(SIZE)]
        target = 0
        mergeable = False  # Can the tile just placed still accept a merge?
        for r, c in order:
            rank = board[r][c]
            if not rank:
                continue
            if mergeable and rank < MAX_RANK and result.board[order[target - 1][0]][order[target - 1][1]] == rank:
                dst = order[target - 1]
                result.board[dst[0]][dst[1]] = rank + 1
                result.merged.append(dst)
                result.gained += tile_value(rank + 1)
                mergeable = False
            else:
                dst = order[target]
                result.board[dst[0]][dst[1]] = rank
                target += 1
                mergeable = True
            result.slides.append(((r, c), dst, rank))
    return result


def has_moves(board: Sequence[Sequence[int]]) -> bool:
    """False only when the grid is full and no neighbours can combine."""
    for r in range(SIZE):
        for c in range(SIZE):
            rank = board[r][c]
            if not rank:
                return True
            if rank < MAX_RANK:
                if c + 1 < SIZE and board[r][c + 1] == rank:
                    return True
                if r + 1 < SIZE and board[r + 1][c] == rank:
                    return True
    return False


class Game:
    def __init__(self, rng: Optional[random.Random] = None, best: int = 0):
        self.rng = rng or random.Random()
        self.best = best
        self.new()

    def new(self) -> None:
        self.board: Board = [[0] * SIZE for _ in range(SIZE)]
        self.score = 0
        self.won = False  # Top tile made; the win message waits for acknowledgement.
        self.continued = False
        self.spawn()
        self.spawn()

    def spawn(self) -> Optional[Cell]:
        empty = [(r, c) for r in range(SIZE) for c in range(SIZE) if not self.board[r][c]]
        if not empty:
            return None
        r, c = self.rng.choice(empty)
        self.board[r][c] = 2 if self.rng.random() < 0.1 else 1
        return r, c

    @property
    def over(self) -> bool:
        return not has_moves(self.board)

    @property
    def highest(self) -> int:
        return max(max(row) for row in self.board)

    @property
    def showing_win(self) -> bool:
        return self.won and not self.continued

    def move(self, direction: str) -> Optional[Tuple[MoveResult, Optional[Cell]]]:
        """Returns the move and the spawned cell, or None if nothing could move."""
        result = slide(self.board, direction)
        if not result.changed:
            return None
        self.board = result.board
        self.score += result.gained
        self.best = max(self.best, self.score)
        if not self.won and self.highest == MAX_RANK:
            self.won = True
        return result, self.spawn()


# ---------------------------------------------------------------- input

_MOUSE = re.compile(rb"\x1b\[<(\d+);(\d+);(\d+)([Mm])")
_CSI = re.compile(rb"\x1b\[[0-9;<?]*[@-~]")
_SS3 = re.compile(rb"\x1bO[@-~]")
_ARROWS = {b"A": "up", b"B": "down", b"C": "right", b"D": "left"}
_KEYS = {b"q": "quit", b"Q": "quit", b"\x03": "quit", b"n": "new", b"N": "new", b"c": "continue", b"C": "continue"}


def parse_input(data: bytes) -> Tuple[list, bytes]:
    """Turns raw terminal bytes into events; returns (events, unconsumed bytes)."""
    events: list = []
    i = 0
    while i < len(data):
        if data[i] != 0x1b:
            key = _KEYS.get(data[i:i + 1])
            if key:
                events.append(key)
            i += 1
            continue
        mouse = _MOUSE.match(data, i)
        if mouse:
            button = int(mouse.group(1))
            if button == 0:  # Primary button only: ignores the wheel and motion.
                kind = "press" if mouse.group(4) == b"M" else "release"
                events.append((kind, int(mouse.group(2)), int(mouse.group(3))))
            elif button == 3 and mouse.group(4) == b"m":
                events.append(("release", int(mouse.group(2)), int(mouse.group(3))))
            i = mouse.end()
            continue
        seq = _CSI.match(data, i) or _SS3.match(data, i)
        if seq:
            arrow = _ARROWS.get(seq.group()[-1:])
            if arrow:
                events.append(arrow)
            i = seq.end()
            continue
        rest = data[i:]
        if re.fullmatch(rb"\x1b(\[[0-9;<?]*|O)?", rest):
            return events, rest  # Incomplete sequence; wait for more bytes.
        i += 1  # Stray ESC.
    return events, b""


def swipe_direction(x0: int, y0: int, x1: int, y1: int) -> Optional[str]:
    """Maps a drag to a direction. Terminal cells are about twice as tall as wide."""
    dx, dy = x1 - x0, (y1 - y0) * 2
    if max(abs(dx), abs(dy)) < 3:
        return None
    if abs(dx) >= abs(dy):
        return "right" if dx > 0 else "left"
    return "down" if dy > 0 else "up"


# ---------------------------------------------------------------- best score

def best_path() -> Path:
    return Path.home() / ".stardust_best"


def load_best() -> int:
    try:
        return max(0, int(best_path().read_text().strip()))
    except (OSError, ValueError):
        return 0


def save_best(best: int) -> None:
    path = best_path()
    tmp = path.with_name(path.name + ".tmp")
    try:
        tmp.write_text(str(best))
        tmp.replace(path)
    except OSError:
        pass  # A lost best score should never interrupt the game.


# ---------------------------------------------------------------- rendering

def char_width(ch: str) -> int:
    return 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1


def text_width(text: str) -> int:
    return sum(char_width(ch) for ch in text)


class Screen:
    """A cell buffer that redraws only the rows that changed."""

    def __init__(self, width: int, height: int):
        self.width, self.height = width, height
        self.previous: List[Optional[list]] = [None] * height
        self.clear()

    def clear(self) -> None:
        self.cells = [[(" ", None, None, False)] * self.width for _ in range(self.height)]

    def _set(self, x: int, y: int, cell: tuple) -> None:
        row = self.cells[y]
        if row[x][0] == "" and x > 0:  # Overwriting the right half of a wide glyph.
            row[x - 1] = (" ",) + row[x - 1][1:]
        if row[x][0] and char_width(row[x][0]) == 2 and x + 1 < self.width:  # Its left half.
            row[x + 1] = (" ",) + row[x + 1][1:]
        row[x] = cell

    def put(self, x: int, y: int, text: str, fg=None, bg=None, bold=False) -> None:
        if not 0 <= y < self.height:
            return
        for ch in text:
            width = char_width(ch)
            if 0 <= x and x + width <= self.width:
                self._set(x, y, (ch, fg, bg, bold))
                if width == 2:
                    self._set(x + 1, y, ("", fg, bg, bold))
            x += width

    def fill(self, x: int, y: int, w: int, h: int, bg: int) -> None:
        for row in range(y, y + h):
            self.put(x, row, " " * w, None, bg)

    def render(self) -> str:
        out = []
        for y, row in enumerate(self.cells):
            if row == self.previous[y]:
                continue
            self.previous[y] = list(row)
            out.append(f"\x1b[{y + 1};1H")
            style = None
            for x, (ch, fg, bg, bold) in enumerate(row):
                if ch == "":
                    continue
                if (fg, bg, bold) != style:
                    style = (fg, bg, bold)
                    codes = ["0"]
                    if bold:
                        codes.append("1")
                    if fg is not None:
                        codes.append(f"38;5;{fg}")
                    if bg is not None:
                        codes.append(f"48;5;{bg}")
                    out.append(f"\x1b[{';'.join(codes)}m")
                out.append(ch)
                if char_width(ch) == 2:
                    # Reposition so a font's unusual emoji width can't shift the row.
                    out.append(f"\x1b[{y + 1};{x + 3}H")
            out.append("\x1b[0m")
        return "".join(out)


@dataclass(frozen=True)
class Layout:
    tile_w: int
    tile_h: int
    gap_x: int
    gap_y: int
    pad_x: int
    pad_y: int
    panel: bool
    width: int
    height: int

    @property
    def board_w(self) -> int:
        return SIZE * self.tile_w + (SIZE - 1) * self.gap_x + 2 * self.pad_x

    @property
    def board_h(self) -> int:
        return SIZE * self.tile_h + (SIZE - 1) * self.gap_y + 2 * self.pad_y


WIDE = Layout(12, 4, 2, 1, 2, 1, panel=True, width=79, height=22)
COMPACT = Layout(10, 4, 1, 1, 0, 0, panel=False, width=43, height=21)  # Phones.


def choose_layout(width: int, height: int) -> Optional[Layout]:
    for layout in (WIDE, COMPACT):
        if width >= layout.width and height >= layout.height:
            return layout
    return None


BOARD_BG, EMPTY_BG = 235, 238
ACCENT, MUTED = 220, 245


class App:
    def __init__(self, game: Game):
        self.game = game
        self.screen: Optional[Screen] = None
        self.layout: Optional[Layout] = None
        self.size = (0, 0)
        self.origin = (0, 0)
        self.press: Optional[Tuple[int, int]] = None
        self.queued = False  # More input is already waiting to be handled.

    # ----- layout

    def check_size(self) -> bool:
        """Rebuilds the screen after a resize. Returns True if it changed."""
        size = tuple(shutil.get_terminal_size((80, 24)))
        if size == self.size:
            return False
        self.size = size
        self.screen = Screen(*size)
        self.layout = choose_layout(*size)
        if self.layout:
            self.origin = ((size[0] - self.layout.width) // 2, (size[1] - self.layout.height) // 2)
        sys.stdout.write("\x1b[0m\x1b[2J")
        return True

    def board_origin(self) -> Tuple[int, int]:
        ox, oy = self.origin
        return ox, oy + (0 if self.layout.panel else 1)

    def cell_xy(self, r: float, c: float) -> Tuple[int, int]:
        lay = self.layout
        ox, oy = self.board_origin()
        return (
            round(ox + lay.pad_x + c * (lay.tile_w + lay.gap_x)),
            round(oy + lay.pad_y + r * (lay.tile_h + lay.gap_y)),
        )

    # ----- drawing

    def draw_tile(self, x: int, y: int, rank: int, grow: int = 0, small: bool = False) -> None:
        lay, scr, tier = self.layout, self.screen, TIERS[rank - 1]
        w, h = lay.tile_w, lay.tile_h
        if small:  # Spawning: a small tile holding only the glyph.
            scr.fill(x + 2, y + 1, w - 4, h - 2, tier.bg)
            scr.put(x + (w - 2) // 2, y + 1, tier.glyph, tier.fg, tier.bg)
            return
        scr.fill(x - grow, y, w + 2 * grow, h, tier.bg)
        scr.put(x + 1, y, str(rank), tier.fg, tier.bg, bold=True)
        scr.put(x + (w - 2) // 2, y + 1, tier.glyph, tier.fg, tier.bg)
        scr.put(x + (w - text_width(tier.name)) // 2, y + 2, tier.name, tier.fg, tier.bg, bold=True)

    def draw_chrome(self) -> None:
        lay, scr, game = self.layout, self.screen, self.game
        ox, oy = self.origin
        bx, by = self.board_origin()
        scr.clear()
        if lay.panel:
            px = ox + lay.board_w + 2
            scr.put(px, oy, "S T A R D U S T", ACCENT, bold=True)
            scr.put(px, oy + 2, "SCORE", MUTED)
            scr.put(px + 7, oy + 2, str(game.score), bold=True)
            scr.put(px, oy + 3, "BEST", MUTED)
            scr.put(px + 7, oy + 3, str(game.best), bold=True)
            scr.put(px, oy + 5, "Smallest to largest", MUTED)
            highest = game.highest
            for i, tier in enumerate(TIERS):
                rank = i + 1
                reached = rank <= highest
                row = oy + 6 + i
                scr.put(px, row, f"{rank:>2}", ACCENT if rank == highest else MUTED)
                scr.put(px + 3, row, tier.glyph)
                scr.put(px + 6, row, tier.name, 255 if reached else MUTED, bold=rank == highest)
        else:
            scr.put(ox, oy, "STARDUST", ACCENT, bold=True)
            status = f"Score {game.score}  Best {game.best}"
            scr.put(ox + lay.width - len(status), oy, status)
        controls = "Arrows or swipe: move   n: new   q: quit"
        scr.put(ox + (lay.board_w - len(controls)) // 2, by + lay.board_h, controls, MUTED)
        scr.fill(bx, by, lay.board_w, lay.board_h, BOARD_BG)
        for r in range(SIZE):
            for c in range(SIZE):
                x, y = self.cell_xy(r, c)
                scr.fill(x, y, lay.tile_w, lay.tile_h, EMPTY_BG)

    def draw_overlay(self, lines: Sequence[Tuple[str, bool]]) -> None:
        lay, scr = self.layout, self.screen
        bx, by = self.board_origin()
        w = min(lay.board_w - 2, max(text_width(text) for text, _ in lines) + 6)
        h = len(lines) + 2
        x = bx + (lay.board_w - w) // 2
        y = by + (lay.board_h - h) // 2
        scr.fill(x, y, w, h, 17)
        for i, (text, strong) in enumerate(lines):
            tx = x + (w - text_width(text)) // 2
            scr.put(tx, y + 1 + i, text, ACCENT if strong else 255, 17, bold=strong)

    def compose(self, tiles=None, pops=(), small=None) -> None:
        """Paints one frame. `tiles` overrides the resting board (mid-animation)."""
        game = self.game
        self.draw_chrome()
        if tiles is not None:
            for x, y, rank in tiles:
                self.draw_tile(x, y, rank)
            return
        for r in range(SIZE):
            for c in range(SIZE):
                rank = game.board[r][c]
                if rank and (r, c) != small and (r, c) not in pops:
                    self.draw_tile(*self.cell_xy(r, c), rank)
        for r, c in pops:  # Drawn last so a popping tile sits above its neighbours.
            self.draw_tile(*self.cell_xy(r, c), game.board[r][c], grow=1)
        if small:
            self.draw_tile(*self.cell_xy(*small), game.board[small[0]][small[1]], small=True)

    def flush(self) -> None:
        sys.stdout.write(self.screen.render())
        sys.stdout.flush()

    def draw(self) -> None:
        if not self.layout:
            self.screen.clear()
            msg = f"Enlarge the terminal to {COMPACT.width}x{COMPACT.height}"
            self.screen.put(max(0, (self.size[0] - len(msg)) // 2), self.size[1] // 2, msg)
            self.flush()
            return
        self.compose()
        game = self.game
        if game.showing_win:
            self.draw_overlay([
                ("You made a Universe!", True),
                (f"Score {game.score}", False),
                ("c: keep going", False),
                ("n: new game   q: quit", False),
            ])
        elif game.over:
            self.draw_overlay([
                ("No moves left", True),
                (f"Score {game.score}   Best {game.best}", False),
                ("n: new game   q: quit", False),
            ])
        self.flush()

    # ----- animation

    def frame(self, seconds: float) -> bool:
        """Shows a frame. Returns False if input arrived, which skips the animation."""
        self.flush()
        return not select.select([sys.stdin], [], [], seconds)[0]

    def animate(self, result: MoveResult, spawned: Optional[Cell]) -> None:
        if not self.layout or self.queued:
            return
        steps = 6
        for step in range(1, steps + 1):
            t = step / steps
            t = 1 - (1 - t) ** 2  # Ease out.
            tiles = []
            for (r0, c0), (r1, c1), rank in result.slides:
                tiles.append((*self.cell_xy(r0 + (r1 - r0) * t, c0 + (c1 - c0) * t), rank))
            self.compose(tiles=tiles)
            if not self.frame(0.014):
                return
        if result.merged:
            self.compose(pops=result.merged, small=spawned)
            if not self.frame(0.06):
                return
        if spawned:
            self.compose(small=spawned)
            self.frame(0.05)

    # ----- events

    def do_move(self, direction: str) -> None:
        game = self.game
        if game.showing_win or game.over:
            return
        before = game.best
        moved = game.move(direction)
        if not moved:
            return
        if game.best != before:
            save_best(game.best)
        self.animate(*moved)

    def handle(self, event) -> bool:
        """Returns False when the game should exit."""
        if isinstance(event, tuple):
            kind, x, y = event
            if kind == "press":
                self.press = (x, y)
            elif self.press:
                direction = swipe_direction(*self.press, x, y)
                self.press = None
                if direction:
                    self.do_move(direction)
        elif event == "quit":
            return False
        elif event == "new":
            self.game.new()
        elif event == "continue":
            if self.game.showing_win:
                self.game.continued = True
        else:
            self.do_move(event)
        return True

    def run(self) -> None:
        fd = sys.stdin.fileno()
        pending = b""
        dirty = True
        while True:
            if self.check_size():
                dirty = True
            if dirty:
                self.draw()
                dirty = False
            if not select.select([fd], [], [], 0.1)[0]:
                pending = b""  # A lone Escape key; no sequence is coming.
                continue
            data = os.read(fd, 1024)
            if not data:
                return
            events, pending = parse_input(pending + data)
            for i, event in enumerate(events):
                self.queued = i < len(events) - 1
                if not self.handle(event):
                    return
                dirty = True


# ---------------------------------------------------------------- terminal setup

ENTER = "\x1b[?1049h\x1b[?25l\x1b[?7l\x1b[?1000h\x1b[?1006h\x1b[2J"
LEAVE = "\x1b[?1006l\x1b[?1000l\x1b[?7h\x1b[?25h\x1b[0m\x1b[?1049l"


def run_app(app: App) -> None:
    fd = sys.stdin.fileno()
    saved = termios.tcgetattr(fd)
    mode = termios.tcgetattr(fd)
    # No echo or line buffering. ISIG stays off so Ctrl-C arrives as a quit key
    # and Ctrl-Z can't suspend the game with the terminal still in this mode.
    mode[3] &= ~(termios.ECHO | termios.ICANON | termios.ISIG)
    mode[6][termios.VMIN] = 1
    mode[6][termios.VTIME] = 0

    def terminate(signum, frame):
        raise SystemExit(128 + signum)

    signal.signal(signal.SIGTERM, terminate)
    signal.signal(signal.SIGHUP, terminate)
    try:
        termios.tcsetattr(fd, termios.TCSADRAIN, mode)
        sys.stdout.write(ENTER)
        sys.stdout.flush()
        app.run()
    finally:
        sys.stdout.write(LEAVE)
        sys.stdout.flush()
        termios.tcsetattr(fd, termios.TCSADRAIN, saved)
        save_best(app.game.best)


def main() -> int:
    parser = argparse.ArgumentParser(description="Merge cosmic matter from Dust up to a Universe.")
    parser.add_argument("--seed", type=int, help="seed the random tile placement")
    args = parser.parse_args()
    if not (sys.stdin.isatty() and sys.stdout.isatty()):
        print("Stardust needs an interactive terminal.", file=sys.stderr)
        return 1
    sys.stdout.reconfigure(encoding="utf-8")
    game = Game(random.Random(args.seed), best=load_best())
    run_app(App(game))
    print(f"Thanks for playing. Score {game.score}, best {game.best}.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
