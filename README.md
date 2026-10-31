# Stardust

[![CI](https://github.com/jklafka/stardust/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/jklafka/stardust/actions/workflows/ci.yml)

A game in the style of 2048, for your terminal or your browser. Slide tiles around a 4x4 grid and merge matching ones to build your way from a speck of Dust to a whole Universe.

## Getting started

### In a terminal

You need [Rust](https://www.rust-lang.org/tools/install) (recent stable) on macOS or Linux, and a terminal with 256-color and emoji support. Windows isn't supported in the terminal.

```sh
git clone https://github.com/jklafka/stardust.git
cd stardust
cargo run --release
```

The built game is at `target/release/stardust`, and it runs without Rust installed.

### In a browser

The web version runs the same game rules as WebAssembly, with swipe controls for phones. To build it you need [rustup](https://rustup.rs) (Homebrew's `rust` formula can't compile to WebAssembly), and the page must be served over HTTP rather than opened as a file:

```sh
rustup toolchain install stable --target wasm32-unknown-unknown
./web/build.sh
python3 -m http.server -d web/www 8000
```

Then open <http://localhost:8000>. After building, `web/www` is a complete static site that any web host can serve.

## How to play

Every move slides all the tiles as far as they can go in one direction. When two matching tiles collide, they merge into the next tile up, and a new Dust or Rock tile then appears in an empty cell.

Make a Universe to win, then keep playing for a higher score if you like. The game ends when the grid is full and no neighboring tiles can merge.

| Input | Action |
|---|---|
| Arrow keys | Move tiles |
| Mouse drag or swipe | Move tiles in the direction of the drag |
| `n` | Start a new game immediately |
| `c` | Keep playing after winning |
| `q` or Ctrl-C | Quit (terminal only) |

In a terminal, swipes only work if your terminal app sends touch as mouse input, which depends on the app. While the terminal version is running, it captures the mouse, so you can't select text in the usual way.

### Tiles

Merging two tiles of one rank makes one tile of the next rank, and scores the points shown for the new tile. A Universe is the top tile, so two of them can't merge.

| Rank | Tile | Points |
|---:|---|---:|
| 1 | ✨ Dust | Starting tile |
| 2 | 🪨 Rock | 2 |
| 3 | 🌠 Comet | 4 |
| 4 | 🌕 Moon | 8 |
| 5 | 🌍 Planet | 16 |
| 6 | 🪐 Gas Giant | 32 |
| 7 | ⭐ Star | 64 |
| 8 | 🔴 Red Giant | 128 |
| 9 | ⚫ Black Hole | 256 |
| 10 | 🌀 Galaxy | 512 |
| 11 | 🌌 Universe | 1024 |

New tiles are Dust 90% of the time and Rock 10% of the time.

### Good to know

- **Best score:** the terminal version saves your best score in `~/.stardust_best`, and the web version saves it in your browser. The two aren't linked.
- **Terminal size:** the full layout, with a side panel listing every tile, needs 79x22. Narrower terminals such as phones get a compact layout that needs 43x21.
- **Repeatable games:** in the terminal, `stardust --seed 7` (or `cargo run --release -- --seed 7`) gives the same tile placements each time if you make the same moves.

## Development

```sh
cargo fmt                                  # Format
cargo test                                 # Run the unit tests
cargo clippy --all-targets -- -D warnings  # Lint
```

[CI](.github/workflows/ci.yml) checks formatting, runs the tests on Ubuntu and macOS, and builds the web version, for every pull request and every push to `main`.

The repo is a Cargo workspace, with the game rules shared by both versions:

| Path | Responsibility |
|---|---|
| `core/src/lib.rs` | Rules, tiles, scoring and spawning, shared by both versions |
| `terminal/src/input.rs` | Key and mouse parsing, and swipe detection |
| `terminal/src/screen.rs` | Cell buffer that redraws only changed rows |
| `terminal/src/app.rs` | Layouts, drawing, animation and the main loop |
| `terminal/src/terminal.rs` | Raw mode, terminal size, input and signals |
| `terminal/src/best.rs` | Loading and saving the best score |
| `terminal/src/main.rs` | Argument parsing and startup |
| `web/src/lib.rs` | Functions exported to the page from WebAssembly |
| `web/www/` | The page: HTML, CSS and JavaScript |
| `web/build.sh` | Compiles the game into `web/www/stardust.wasm` |

`libc` is the only dependency, used by the terminal version. The web version has no build tools beyond Rust, and no JavaScript dependencies.

Contributors and coding agents should also read [AGENTS.md](AGENTS.md), which covers the rules to preserve, how to test in a real terminal, and the Git workflow.
