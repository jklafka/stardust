# AGENTS.md

Stardust is a game written in Rust: tiles slide on a 4x4 grid, and two matching tiles merge into the next of 11 tiers, from the lowest tier up to the top one (Dust to Universe, or Seed to Oak in the Plants theme). It has a terminal version for Unix, which uses `termios` through `libc`, and a web version that compiles the same rules to WebAssembly. The web version is live at https://stardust.jklafka.cloud.

## Commands

```sh
cargo run --release                       # Play in the terminal
cargo fmt                                 # Format
cargo test                                # Run all tests
cargo clippy --all-targets -- -D warnings # Lint
./web/build.sh                            # Build the web version into web/www
python3 -m http.server -d web/www 8000    # Serve it at http://localhost:8000
```

`web/build.sh` needs rustup with the `wasm32-unknown-unknown` target. Homebrew's `rust` formula can't compile to WebAssembly.

## Code layout

| File | Responsibility |
|---|---|
| `core/src/lib.rs` | Rules, tiers, themes, scoring, spawning. No terminal or web code. |
| `terminal/src/input.rs` | Turns raw bytes into events and drags into swipes. |
| `terminal/src/screen.rs` | Cell buffer that redraws only changed rows. |
| `terminal/src/app.rs` | Layouts, drawing, animation, main loop. |
| `terminal/src/terminal.rs` | All `libc` and `unsafe` code: raw mode, size, input, signals. |
| `terminal/src/best.rs` | Loads and saves the best score. |
| `terminal/src/theme.rs` | Loads and saves the chosen theme in `~/.stardust_theme`. |
| `terminal/src/main.rs` | Argument parsing and startup. |
| `web/src/lib.rs` | Plain functions exported from WebAssembly to the page. |
| `web/www/app.js` | Page logic: rendering, animation, keys, swipes, saved game, best score and theme. |
| `web/www/style.css`, `index.html` | The page itself, including the theme palettes. |
| `web/www/404.html` | The page served for unknown paths. Standalone, with its own styles. |
| `web/build.sh` | Compiles the game to `web/www/stardust.wasm`, which is gitignored. |

Keep game rules in `core` so both versions share them and they can be tested without a terminal or browser, and keep `unsafe` in `terminal/src/terminal.rs`. Tests live in a `#[cfg(test)]` module in the file they cover.

`web/src/lib.rs` and `web/www/app.js` are two sides of one interface, including the packed integers for moves, so change them together. Nothing type-checks that interface, and `cargo test` doesn't run the page.

## Rules to preserve

**Game**
- Each tile merges at most once per move.
- Top-tier tiles never merge, and adjacent ones don't count as an available move.
- The game ends only when the grid is full and no neighbors can merge.
- A blocked move changes nothing and spawns no tile.
- Making the top tile shows a win message, and the player can keep playing.

**Themes**
- Themes change only names, glyphs, colors and the win message, never the rules.
- Switching is allowed only before the first move, while the win message shows, or when the game is over (`Game::can_switch_theme`).
- Every glyph must meet the `char_width` rule.
- Adding a theme needs a CSS palette in `web/www/style.css` too.

**Best score**
- `~/.stardust_best` holds a plain integer. Keep that format so existing scores still load.
- Save failures must never interrupt the game.

**Terminal**
- While the game is running, write output through `terminal::write_out`, not `print!` or `println!`.
- `RawTerminal` must restore the terminal on every exit path, including signals and panics. Signal handlers only store to an atomic.
- Ctrl-C arrives as a key because `ISIG` is off, and Ctrl-Z deliberately does nothing.
- Keep `select` for waiting on input. See the comment on `input_ready`.

**Drawing**
- `char_width` only knows the characters the game draws. When adding an emoji, add it there if it isn't already covered, and use a single code point that is always double width, with no variation selector.
- The wide layout must fit a default 80x24 terminal, and the compact layout is for narrow terminals such as phones. Sizes are the `WIDE` and `COMPACT` constants in `terminal/src/app.rs`.

**Input and animation**
- Queued input skips animation, so fast play never lags.
- Keep motion calm. The maintainer finds fast or scaling animation uncomfortable, so use no scale, bounce or flash. Web slides take about 200ms.

**Web**
- Tiles must stay square at every screen size, including phones. Their width and height use the same formula so content can't stretch them.
- The page uses no build step and no JavaScript dependencies. It loads the module with plain `WebAssembly.instantiate`.
- `localStorage` can be blocked, so best-score failures must never interrupt the game.
- Saved games in `localStorage` key `stardust-game` belong to real players. A format change must keep loading older versions (currently v2, which still accepts v1), and an invalid save must start a fresh game rather than crash.

## Dependencies

`libc` is the only dependency, used by the terminal version. Ask the maintainer before adding a crate or a JavaScript package.

## Checking changes in a terminal

Unit tests don't cover drawing or terminal handling, so run the game for those changes. This uses tmux with a temporary `HOME`, which keeps tests from overwriting the player's real best score:

```sh
cargo build --release
tmux -L stardust-test -f /dev/null new-session -d -x 80 -y 24 -s game \
  "env HOME=$(mktemp -d) LANG=en_US.UTF-8 $PWD/target/release/stardust --seed 7"
tmux -L stardust-test send-keys -t game Left   # Send a key
tmux -L stardust-test capture-pane -pt game    # Show the screen (add -e for colors)
tmux -L stardust-test kill-server
```

Also check the compact layout with `-x 46 -y 25`. tmux can't show how colors, emoji and animation actually look, so say so in the PR when a change depends on them.

## Checking changes in a browser

Run web changes in a real browser, at a desktop size and a phone size, and try both keys and touch. Chrome's device emulation is useful, but it isn't the same as a real phone, so say so in the PR if you haven't used one.

## Deployment

The web version is live at https://stardust.jklafka.cloud. It's a Cloudflare Pages project named `stardust` (`stardust-dww.pages.dev`), connected to this repo. Every merge to `main` redeploys automatically, so merging ships to players. Each pull request gets a Cloudflare preview at `<branch>.stardust-dww.pages.dev`.

- Build output directory: `web/www`
- Build command: `curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --target wasm32-unknown-unknown && . "$HOME/.cargo/env" && ./web/build.sh`
- Build settings live in the Cloudflare dashboard, not in this repo.
- `web/www/404.html` is served for unknown paths, so its links must be root-absolute (such as `/`).

## Git workflow

GitHub Actions (`.github/workflows/ci.yml`) runs `cargo fmt --check`, `cargo test` on Ubuntu and macOS, and the web build, on every PR and every push to `main`. It doesn't run clippy or test the page in a browser, and `main` has no branch protection, so GitHub won't block a merge on its own. Never push directly to `main`.

1. Work on a branch.
2. Before opening a PR, make sure `cargo fmt --check`, `cargo test` and `cargo clippy --all-targets -- -D warnings` pass. For drawing, terminal or web changes, also do the matching check above.
3. Open a PR that explains what changed, what was tested, and what wasn't.
4. Wait for CI (`gh pr checks --watch`). Once it passes, you may merge without further sign-off: `gh pr merge --squash --delete-branch`

Commit `Cargo.lock`. Keep plans and feature ideas in GitHub Issues, not in this file.
