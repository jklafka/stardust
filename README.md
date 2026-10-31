# Stardust

[![CI](https://github.com/jklafka/stardust/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/jklafka/stardust/actions/workflows/ci.yml)

**[Play Stardust in your browser at stardust.jklafka.cloud](https://stardust.jklafka.cloud)**

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

Play the live site at <https://stardust.jklafka.cloud>. It has swipe controls for phones, and your game in progress is saved in the browser, so a refresh doesn't lose it.

To run your own copy, the web version compiles the same game rules to WebAssembly. You need [rustup](https://rustup.rs) (Homebrew's `rust` formula can't compile to WebAssembly), and the page must be served over HTTP rather than opened as a file:

```sh
rustup toolchain install stable --target wasm32-unknown-unknown
./web/build.sh
python3 -m http.server -d web/www 8000
```

Then open <http://localhost:8000>. After building, `web/www` is a complete static site that any web host can serve.

## How to play

Every move slides all the tiles as far as they can go in one direction. When two matching tiles collide, they merge into the next tile up, and a new Dust or Rock tile (Seed or Sprout in the Plants theme, Bard or Rogue in the Fantasy theme) then appears in an empty cell.

Make a Universe (an Oak in the Plants theme, a Wizard in the Fantasy theme) to win, then keep playing for a higher score if you like. The game ends when the grid is full and no neighboring tiles can merge.

| Input | Action |
|---|---|
| Arrow keys, `w` `a` `s` `d`, or `i` `j` `k` `l` (either case) | Move tiles: `w`/`i` up, `s`/`k` down, `a`/`j` left, `d`/`l` right |
| Mouse drag or swipe | Move tiles in the direction of the drag |
| `n` | Start a new game immediately |
| `c` | Keep playing after winning |
| `t` | Change theme. Only works before the first move, while the win message shows, or after the game ends |
| `q` or Ctrl-C | Quit (terminal only) |

In a terminal, swipes only work if your terminal app sends touch as mouse input, which depends on the app. While the terminal version is running, it captures the mouse, so you can't select text in the usual way.

### Tiles

Merging two tiles of one rank makes one tile of the next rank, and scores the points shown for the new tile. The top tile can't merge, so two of them can't combine.

The game has three themes, Space, Plants and Fantasy, which change only the names and pictures of the tiles. Press `t` to switch, when the game allows it. The points are the same in every theme.

#### Space

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

#### Plants

| Rank | Tile | Points |
|---:|---|---:|
| 1 | 🌰 Seed | Starting tile |
| 2 | 🌱 Sprout | 2 |
| 3 | 🍀 Clover | 4 |
| 4 | 🌿 Herb | 8 |
| 5 | 🌷 Tulip | 16 |
| 6 | 🌻 Sunflower | 32 |
| 7 | 🎍 Bamboo | 64 |
| 8 | 🌵 Cactus | 128 |
| 9 | 🌴 Palm | 256 |
| 10 | 🌲 Pine | 512 |
| 11 | 🌳 Oak | 1024 |

#### Fantasy

The web version draws each class as a 16-bit style pixel-art sprite. The terminal's tiles are too small for sprites, so it uses the emoji shown here.

| Rank | Tile | Points |
|---:|---|---:|
| 1 | 🎻 Bard | Starting tile |
| 2 | 🥷 Rogue | 2 |
| 3 | 🤺 Fighter | 4 |
| 4 | 🏹 Ranger | 8 |
| 5 | 🥋 Monk | 16 |
| 6 | 🪓 Barbarian | 32 |
| 7 | 🌿 Druid | 64 |
| 8 | 📿 Cleric | 128 |
| 9 | 💀 Warlock | 256 |
| 10 | 🏰 Paladin | 512 |
| 11 | 🧙 Wizard | 1024 |

New tiles are the first tile 90% of the time and the second 10% of the time (Dust or Rock in Space).

### Good to know

- **Best score:** the terminal version saves your best score in `~/.stardust_best`, and the web version saves it in your browser. The two aren't linked. Either way, one best score is shared across all themes.
- **Theme:** the terminal version remembers your theme in `~/.stardust_theme`, and the web version remembers it in your browser.
- **Saved game:** the web version saves the game in progress in your browser, so it's still there after a refresh. The terminal version doesn't save games, so quitting ends the current one.
- **Terminal size:** the full layout, with a side panel listing every tile, needs 71x24. Narrower terminals such as phones get a compact layout that needs 46x25.
- **Repeatable games:** in the terminal, `stardust --seed 7` (or `cargo run --release -- --seed 7`) gives the same tile placements each time if you make the same moves.

## Development

```sh
cargo fmt                                  # Format
cargo test                                 # Run the unit tests
cargo clippy --all-targets -- -D warnings  # Lint
```

[CI](.github/workflows/ci.yml) checks formatting, runs the tests on Ubuntu and macOS, and builds the web version, for every pull request and every push to `main`.

The live site is deployed from `main` on Cloudflare Pages, so merging a pull request updates https://stardust.jklafka.cloud. [AGENTS.md](AGENTS.md) has the details.

The repo is a Cargo workspace, with the game rules shared by both versions:

| Path | Responsibility |
|---|---|
| `core/src/lib.rs` | Rules, tiles, themes, scoring and spawning, shared by both versions |
| `terminal/src/input.rs` | Key and mouse parsing, and swipe detection |
| `terminal/src/screen.rs` | Cell buffer that redraws only changed rows |
| `terminal/src/app.rs` | Layouts, drawing, animation and the main loop |
| `terminal/src/terminal.rs` | Raw mode, terminal size, input and signals |
| `terminal/src/best.rs` | Loading and saving the best score |
| `terminal/src/theme.rs` | Loading and saving the chosen theme |
| `terminal/src/main.rs` | Argument parsing and startup |
| `web/src/lib.rs` | Functions exported to the page from WebAssembly |
| `web/src/sprites.rs` | Pixel art for the Fantasy theme |
| `web/www/app.js` | Page logic: rendering, animation, keys, swipes, saved game, best score and theme |
| `web/index.template.html`, `web/www/style.css` | The page itself, including the theme colors |
| `web/www/404.html` | The page shown for unknown addresses |
| `web/build.sh` | Compiles the game and generates `web/www/stardust.wasm` and `web/www/index.html` |

`libc` is the only dependency, used by the terminal version. The web version has no build tools beyond Rust, and no JavaScript dependencies.

Contributors and coding agents should also read [AGENTS.md](AGENTS.md), which covers the rules to preserve, how to test in a real terminal, and the Git workflow.
