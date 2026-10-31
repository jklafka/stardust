# AGENTS.md

Stardust is a terminal game written in Rust: tiles slide on a 4x4 grid, and two matching tiles merge into the next of 11 tiers, from Dust up to Universe. It targets Unix terminals and uses `termios` through `libc`.

## Commands

```sh
cargo run --release                       # Play
cargo test                                # Run all tests
cargo clippy --all-targets -- -D warnings # Lint
```

Do not use `cargo fmt` as a check yet. The code is not rustfmt-clean, so running it rewrites unrelated lines. Match the surrounding style, and keep any repo-wide reformat in its own PR.

## Code layout

| File | Responsibility |
|---|---|
| `src/game.rs` | Rules, tiers, scoring, spawning. No terminal code. |
| `src/input.rs` | Turns raw bytes into events and drags into swipes. |
| `src/screen.rs` | Cell buffer that redraws only changed rows. |
| `src/app.rs` | Layouts, drawing, animation, main loop. |
| `src/terminal.rs` | All `libc` and `unsafe` code: raw mode, size, input, signals. |
| `src/best.rs` | Loads and saves the best score. |
| `src/main.rs` | Argument parsing and startup. |

Keep game rules in `game.rs` so they can be tested without a terminal, and keep `unsafe` in `terminal.rs`. Tests live in a `#[cfg(test)]` module in the file they cover.

## Rules to preserve

**Game**
- Each tile merges at most once per move.
- Top-tier tiles never merge, and adjacent ones don't count as an available move.
- The game ends only when the grid is full and no neighbors can merge.
- A blocked move changes nothing and spawns no tile.
- Making the top tile shows a win message, and the player can keep playing.

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
- The wide layout must fit a default 80x24 terminal, and the compact layout is for narrow terminals such as phones. Sizes are the `WIDE` and `COMPACT` constants in `src/app.rs`.

**Input and animation**
- Queued input skips animation, so fast play never lags.

## Dependencies

`libc` is the only dependency. Ask the maintainer before adding a crate.

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

Also check the compact layout with `-x 45 -y 24`. tmux can't show how colors, emoji and animation actually look, so say so in the PR when a change depends on them.

## Git workflow

There is no CI and `main` has no branch protection, so these checks are the only gate. Never push directly to `main`.

1. Work on a branch.
2. Before merging, make sure `cargo test` and `cargo clippy --all-targets -- -D warnings` pass. For drawing or terminal changes, also do the terminal check above.
3. Open a PR that explains what changed, what was tested, and what wasn't.
4. Once the checks pass, you may merge without further sign-off: `gh pr merge --squash --delete-branch`

Commit `Cargo.lock`. Keep plans and feature ideas in GitHub Issues, not in this file.
