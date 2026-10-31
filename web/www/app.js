// Connects the page to the game compiled from Rust. `web/src/lib.rs` is the
// other side of this interface, so change the two together.

const SIZE = 4;
const SLIDE_MS = 110; // Keep in step with --slide-ms in style.css.
const SWIPE_MIN_PX = 24;
const BEST_KEY = "stardust-best";
const GAME_KEY = "stardust-game";
const GAME_VERSION = 1;
const DIRECTIONS = {
  ArrowUp: 0, ArrowDown: 1, ArrowLeft: 2, ArrowRight: 3,
  w: 0, W: 0, i: 0, I: 0,
  s: 1, S: 1, k: 1, K: 1,
  a: 2, A: 2, j: 2, J: 2,
  d: 3, D: 3, l: 3, L: 3,
};

const byId = (id) => document.getElementById(id);
const board = byId("board");
const tilesLayer = byId("tiles");
const overlay = byId("overlay");
const keepGoing = byId("keep-going");
const playAgain = byId("play-again");
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");

let game;
let tiers = [];
let settleTimer = null;
let pendingEffects = null;
let overlayKind = null;

function loadBest() {
  try {
    return Number.parseInt(localStorage.getItem(BEST_KEY), 10) || 0;
  } catch {
    return 0; // Storage can be blocked; the game still works without it.
  }
}

function saveBest(best) {
  try {
    localStorage.setItem(BEST_KEY, String(best));
  } catch {
    // A lost best score should never interrupt the game.
  }
}

function saveGame() {
  try {
    const cells = [];
    for (let r = 0; r < SIZE; r += 1) {
      for (let c = 0; c < SIZE; c += 1) cells.push(game.stardust_cell(r, c));
    }
    localStorage.setItem(
      GAME_KEY,
      JSON.stringify({
        v: GAME_VERSION,
        board: cells,
        score: game.stardust_score(),
        won: game.stardust_won() === 1,
        continued: game.stardust_continued() === 1,
      }),
    );
  } catch {
    // A game that isn't saved should never interrupt play.
  }
}

// Restores the saved game if it is valid. Returns whether it did. Ranges of
// the board cells are checked in Rust; this only checks the shape and types.
function restoreGame() {
  try {
    const saved = JSON.parse(localStorage.getItem(GAME_KEY));
    if (!saved || saved.v !== GAME_VERSION) return false;
    const { board: cells, score, won, continued } = saved;
    if (!Array.isArray(cells) || cells.length !== SIZE * SIZE) return false;
    if (!cells.every((rank) => Number.isInteger(rank) && rank >= 0)) return false;
    if (!Number.isInteger(score) || score < 0 || score > 0xffffffff) return false;
    if (typeof won !== "boolean" || typeof continued !== "boolean") return false;
    game.stardust_restore_begin();
    cells.forEach((rank, index) => game.stardust_restore_cell(index, rank));
    return game.stardust_restore_commit(score, won ? 1 : 0, continued ? 1 : 0) === 1;
  } catch {
    return false;
  }
}

function readTiers(memory) {
  const decoder = new TextDecoder();
  return Array.from({ length: game.stardust_tier_count() }, (_, tier) => ({
    glyph: String.fromCodePoint(game.stardust_tier_glyph(tier)),
    name: decoder.decode(
      new Uint8Array(
        memory.buffer,
        game.stardust_tier_name_ptr(tier),
        game.stardust_tier_name_len(tier),
      ),
    ),
  }));
}

function createTile(rank, r, c, effect) {
  const tier = tiers[rank - 1];
  const tile = document.createElement("div");
  tile.className = effect ? `tile ${effect}` : "tile";
  tile.style.setProperty("--tile", `var(--tile-${rank})`);
  tile.style.setProperty("--ink", `var(--ink-${rank})`);
  placeTile(tile, r, c);

  for (const [className, text] of [
    ["rank", rank],
    ["glyph", tier.glyph],
    ["name", tier.name],
  ]) {
    const part = document.createElement("span");
    part.className = className;
    part.textContent = text;
    tile.append(part);
  }
  return tile;
}

function placeTile(tile, r, c) {
  tile.style.setProperty("--r", r);
  tile.style.setProperty("--c", c);
  tile.dataset.cell = `${r},${c}`;
}

function cellKey(packed) {
  return `${packed & 15},${(packed >> 4) & 15}`;
}

// Draws the board at rest, with pops and spawns from the move that just ended.
function renderTiles(effects) {
  const tiles = [];
  for (let r = 0; r < SIZE; r += 1) {
    for (let c = 0; c < SIZE; c += 1) {
      const rank = game.stardust_cell(r, c);
      if (rank) {
        const key = `${r},${c}`;
        let effect = null;
        if (effects?.merged.has(key)) effect = "pop";
        else if (effects?.spawned === key) effect = "spawn";
        tiles.push(createTile(rank, r, c, effect));
      }
    }
  }
  tilesLayer.replaceChildren(...tiles);
}

function renderStats() {
  byId("score").textContent = game.stardust_score();
  byId("best").textContent = game.stardust_best();

  let highest = 0;
  for (let r = 0; r < SIZE; r += 1) {
    for (let c = 0; c < SIZE; c += 1) {
      highest = Math.max(highest, game.stardust_cell(r, c));
    }
  }
  [...byId("tier-list").children].forEach((item, index) => {
    item.classList.toggle("reached", index < highest);
    item.classList.toggle("highest", index + 1 === highest);
  });
}

function renderOverlay() {
  const kind = game.stardust_showing_win() ? "win" : game.stardust_over() ? "over" : null;
  if (kind === overlayKind) return;
  const hadFocus = overlay.contains(document.activeElement);
  overlayKind = kind;
  overlay.hidden = !kind;
  if (!kind) {
    if (hadFocus) board.focus();
    return;
  }

  const score = game.stardust_score();
  const title = kind === "win" ? "You made a Universe!" : "No moves left";
  byId("overlay-title").textContent = title;
  byId("overlay-score").textContent =
    kind === "win" ? `Score ${score}` : `Score ${score} · Best ${game.stardust_best()}`;
  keepGoing.hidden = kind !== "win";
  byId("status").textContent = `${title} Score ${score}.`;
  (kind === "win" ? keepGoing : playAgain).focus();
}

// Finishes any running animation, so fast input never waits for one.
function settle() {
  if (settleTimer !== null) {
    clearTimeout(settleTimer);
    settleTimer = null;
  }
  renderTiles(pendingEffects);
  pendingEffects = null;
  renderStats();
  renderOverlay();
}

function move(direction) {
  settle();
  const bestBefore = game.stardust_best();
  if (!game.stardust_try_move(direction)) return;
  saveGame();
  if (game.stardust_best() !== bestBefore) saveBest(game.stardust_best());

  const resting = new Map(
    [...tilesLayer.children].map((tile) => [tile.dataset.cell, tile]),
  );
  for (let i = 0; i < game.stardust_slide_count(); i += 1) {
    const slide = game.stardust_slide(i);
    const tile = resting.get(`${slide & 15},${(slide >> 4) & 15}`);
    if (tile) placeTile(tile, (slide >> 8) & 15, (slide >> 12) & 15);
  }

  const merged = new Set();
  for (let i = 0; i < game.stardust_merged_count(); i += 1) {
    merged.add(cellKey(game.stardust_merged(i)));
  }
  const spawned = game.stardust_spawned();
  pendingEffects = { merged, spawned: spawned < 0 ? null : cellKey(spawned) };

  byId("score").textContent = game.stardust_score();
  byId("best").textContent = game.stardust_best();

  if (reducedMotion.matches) {
    settle();
  } else {
    settleTimer = setTimeout(settle, SLIDE_MS);
  }
}

function newGame() {
  pendingEffects = null;
  game.stardust_new_game();
  saveGame();
  settle();
}

function continuePlaying() {
  game.stardust_continue();
  saveGame();
  settle();
}

function onKeyDown(event) {
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  if (event.key in DIRECTIONS) {
    event.preventDefault();
    move(DIRECTIONS[event.key]);
  } else if (event.key === "n" || event.key === "N") {
    newGame();
  } else if ((event.key === "c" || event.key === "C") && overlayKind === "win") {
    continuePlaying();
  }
}

function watchSwipes() {
  let start = null;
  board.addEventListener("pointerdown", (event) => {
    if (overlayKind || (event.pointerType === "mouse" && event.button !== 0)) return;
    start = { x: event.clientX, y: event.clientY, id: event.pointerId };
    board.setPointerCapture(event.pointerId);
  });
  board.addEventListener("pointerup", (event) => {
    if (!start || event.pointerId !== start.id) return;
    const dx = event.clientX - start.x;
    const dy = event.clientY - start.y;
    start = null;
    if (Math.max(Math.abs(dx), Math.abs(dy)) < SWIPE_MIN_PX) return;
    if (Math.abs(dx) >= Math.abs(dy)) {
      move(dx > 0 ? DIRECTIONS.ArrowRight : DIRECTIONS.ArrowLeft);
    } else {
      move(dy > 0 ? DIRECTIONS.ArrowDown : DIRECTIONS.ArrowUp);
    }
  });
  board.addEventListener("pointercancel", () => {
    start = null;
  });
}

function buildPage() {
  const cells = document.querySelector(".cells");
  cells.append(...Array.from({ length: SIZE * SIZE }, () => document.createElement("div")));
  board.tabIndex = -1;

  byId("tier-list").append(
    ...tiers.map((tier, index) => {
      const item = document.createElement("li");
      const number = document.createElement("span");
      number.className = "number";
      number.textContent = index + 1;
      item.append(number, `${tier.glyph} ${tier.name}`);
      return item;
    }),
  );

  byId("new-game").addEventListener("click", newGame);
  playAgain.addEventListener("click", newGame);
  keepGoing.addEventListener("click", continuePlaying);
  document.addEventListener("keydown", onKeyDown);
  watchSwipes();
}

async function start() {
  const response = await fetch(new URL("stardust.wasm", import.meta.url));
  if (!response.ok) throw new Error(`Could not load the game (HTTP ${response.status}).`);
  const { instance } = await WebAssembly.instantiate(await response.arrayBuffer());
  game = instance.exports;

  const [seed] = crypto.getRandomValues(new Uint32Array(1));
  game.stardust_init(seed, loadBest());
  tiers = readTiers(game.memory);
  if (!restoreGame()) saveGame();
  buildPage();
  settle();
}

start().catch((error) => {
  console.error(error);
  const message = document.createElement("p");
  message.className = "fatal";
  message.textContent = "Stardust couldn't start. Serve this folder over HTTP, and check that web/build.sh has been run.";
  document.querySelector(".app").replaceChildren(message);
});
