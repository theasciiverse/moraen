# Moraen

Asciiverse cave-digging action puzzle. Rust + Ratatui (+ half-blocks for tile art,
the pipeline uXo and Marble Recovery proved). A spiritual successor to Boulder Dash
(Peter Liepa and Chris Gray, First Star Software, 1984): dig through dirt, collect
enough diamonds to open the exit, and survive the boulders, flies and amoeba while
the clock runs. The engine faithful to the original, everything you see our own.
Not a clone.

**Status:** build step 1 done (2026-10-05): the BD1 engine with every BD1 element,
34 `cargo test`s of its rules, the glyph renderer, keyboard and gamepad input, session
logging, and one cave of our own (`caves/01-first-thaw.txt`) played through the five
difficulty levels. Proposals below approved by David on 2026-10-05. Own git repo,
private at github.com/theasciiverse/moraen.

```bash
cargo run --release                       # play (--level 1-5)
RUST_LOG=debug cargo run --release 2>moraen.log
cargo run -- --frame 60 --moves ..RRDD    # headless: N ticks, print the screen
cargo run -- --frame 60 --size 320x75 --ppm out.ppm   # ... or save it as an image
cargo test
```

Code: `src/engine.rs` (the tick, all rules, tests), `src/cave.rs` (cave format),
`src/glyph.rs` (glyph mode), `src/pixel.rs` (pixel mode), `src/input.rs`, `src/main.rs` (session: lives, score,
levels, HUD).

## Name

**Moraen**, chosen by David on 2026-10-04. From the Swedish **morän** (moraine, glacial
till): "Morän är en jordart bildad av en glaciär eller inlandsis, genom en osorterad
avlagring av bergmaterial direkt från glaciäris" (sv.wikipedia, *Morän (jordart)*).
Unsorted rock, from clay to boulders over 60 cm, dumped by the ice; it covers about
75% of Sweden. A cave of loose rock waiting to fall is exactly that. The `ä` becomes
`ae`, the old way of writing it without the dots, which makes the name our own and
keeps it plain ASCII.

- **Moraen**: the name, in-game, on the store page and in prose.
- **Morän** could be the logo: the two dots over the `a` drawn as two boulders.
  To decide when the title screen is made.
- **`moraen`**: crate and repo name. Free on crates.io (`moraine` is taken by an
  active crate); `theasciiverse/moraen` free (checked 2026-10-04). The folder is
  `moraen` too (renamed from the working name `rockford` on 2026-10-04).

Name checks 2026-10-04: no game called Moraen on Steam or itch.io; "Moraine" on Steam
is only a fishing-simulator DLC (*Moraine Lake*). Other Moraens: a minor Gondorian in
Tolkien's late notes, a Swiss musician's stage name, a Minecraft YouTuber, a dormant
personal site repo (`Moraen/moraen.se`, last push 2018, domain not answering). None
are games.

**Trademark search** (TMview, 2026-10-04: EUIPO, USPTO, UK, Swedish PRV and most
national offices). No MORAEN mark anywhere. "Contains" finds five, none near us:
three Korean marks (MORAENG, MoraeNori, MORAENYANG; retail, food, plants), an ended
Korean kids' cafe, and a German gravel company (*omk Oberschwaben Moraene-Kies*,
classes 19/37/39/42, fittingly moraine gravel). Fuzzy: 3,437 hits; the top 300 read,
with classes 9, 28 and 41 picked out. Nothing for games. Nearest in class 9 or 41:
Moreen (Canada, class 9, Shenzhen electronics, 2023), Morain (EU and UK, class 9,
Shenzhen, 2020), MORAE (TechSmith usability software, Japan and Korea), MORAEZ
(Brazil, class 41, filed 2025), MIRAEN (Korean publisher, 9/41, filed 2025). Clear.

Rejected on the way (Steam and itch.io): Scree, Talus, Cave in, Landslip, Rockfall,
Strata (all taken); Bedrock (Minecraft), Lode (Lode Runner), anything with "Cave"
(blurs with CAVEZ of PHEAR); Caret was the runner-up.

## Rights

Checked 2026-10-04, not legal advice.

- **Mechanics are free**, on the same footing as uXo (*Tetris Holding v. Xio*, 2012;
  *SAS Institute v. World Programming*, CJEU C-406/10, 2012).
- **Owner**: BBG Entertainment GmbH bought all Boulder Dash IP from First Star
  Software in 2017 and is actively publishing (Boulder Dash 40th Anniversary,
  2025). Their legal page: "Boulder Dash® and Rockford® are trademarks of BBG
  Entertainment GmbH, registered in the US, the European Union and other
  countries." Crystal and Goldford are also their (unregistered) marks.
- So never in the title, store copy or game: Boulder Dash, Rockford, Crystal,
  Goldford. "A love letter to the 1984 cave-digging classic" says it.
- **The original caves are BBG's copyright.** Never copy a layout, seed table or
  cave parameter set from BD1 to BD3. GDash and the retrointernals disassembly both
  contain them; read the engine code, not the cave data.
- **Our own**: name, hero, creature names and looks, every cave, music and sound.
  The hero must not echo Rockford's look or his signature foot-tapping idle.

## References and what we may take

| Source | License (verified) | Use |
|---|---|---|
| GDash (Czirkos Zoltán) | MIT (COPYING read) | engine code may be lifted; its bundled original caves may not |
| Rocks'n'Diamonds `src/game_bd/*` | ISC (Debian copyright file) | GDash's BD engine as R'n'D embeds it; may be lifted |
| Rocks'n'Diamonds rest | GPL-2+ | ideas only (we are not GPL) |
| R'n'D level sets (artsoft.org) | none stated; Debian: "license for the game data is unclear", removed from the package | never bundled; **import** only (see Modes) |
| Boulder (Ronnie Hedlund, Debian `boulder-game`) | "All code and data-files concerning Boulder are Public Domain" | 25 levels, art and sound free to use; open the `.map` files first to confirm none copies an original cave |
| CAVEZ of PHEAR (Tom Rune Flo) | GPL-3 | a catalog neighbour, not a source |
| C64 BD1 disassembly (retrointernals.org) | none stated | behaviour reference only |

Fan caves we love: ask the author directly for permission to include them.

## Catalog neighbours

**CAVEZ of PHEAR** (2003 to 2011, `external/games/phear-0.5.1`) is the existing
terminal BD-like: one 80x23 glyph screen, ten element types, one monster, bombs
you set off yourself; no flies, amoeba, magic wall, timer or quota. Proposal:
promote it to a catalog title under the OLD! banner (GPL-3 lets us ship it as is)
and make this game the full-engine one beside it, under a clearly different name.
No Rust or Ratatui BD exists (research note, section 4).

## Core rules (the genre, all free)

Exact rules, numbers and sources are in `research/boulder-dash.md`; the summary:

- Cave 40x22 tiles including a steel border (intermissions 20x12).
- **One tick is one scan** of the grid in place, row by row, left to right; a moved
  object is marked scanned so it never moves twice. Scan order decides races. Input
  is read once per tick, when the scan reaches the player.
- Dirt is dug, diamonds collected (stationary ones only), boulders and diamonds fall
  and roll off rounded things (brick, stationary boulders and diamonds; left tried
  first). A falling object kills the player, fireflies and butterflies.
- Push a stationary boulder sideways into space: 1 in 4 per tick of trying
  (disassembly `AND #$03`; Broadribb's "1 in 8" is wrong).
- Snap: grab or dig the neighbouring cell without moving.
- Firefly follows the left wall and explodes to space; butterfly follows the right
  wall and explodes to diamonds; both blow up on touching the player or amoeba.
- Amoeba grows (slow, then fast), becomes boulders at 200 cells, diamonds when
  enclosed. Magic wall, once hit, turns boulders falling through into diamonds and
  back for a limited time. Explosions are 3x3, only steel survives.
- Exit opens at the diamond quota; diamonds past it are worth more; leftover time
  scores per second; extra life every 500 points.
- Five difficulty levels per cave: own seed, quota and time, a seeded random fill
  of up to four (object, chance) pairs over the base.

**Engine stance:** the BD1 tick model exactly, quirks decided one by one and written
down; the classic feel comes from the scan, the 1-in-4 push and the per-tick input,
so those are not "improved".

**Engine decisions** (2026-10-05; each is one constant or a few lines to flip):

- Scanned twins are a per-cell `scanned` bit set on every cell written this tick,
  cleared after the scan. Same behaviour as BD1's twin codes.
- Magic wall: the hit that wakes it also passes through (checked in GDash's
  `do_fall_try_magic`). BD1's **scan abort** after a magic wall hit (the rest of the
  tick skipped, amoeba misjudged as enclosed) is **not** copied; BD2 fixed it too.
- Amoeba slow timer starts at hatching (the disassembly), not at cave start
  (GDash's BD1 setting and the Inside FAQ say immediately). Open to flip.
- Hatching: 4 game seconds (the disassembly; GDash uses 2), then the three
  pre-player stages one per tick. The cave runs during it.
- Explosions: solid four ticks (GDash `short_explosions` for BD1), no chains,
  only steel survives.
- Falling objects crush only a player not yet moved this tick (BD1 tests the
  unscanned code); in a top-down scan this never comes up in play.
- Tick length per level 150/130/120/110/100 ms (intermissions 120 to 70): BD1's
  CaveDelay 12/6/3/1/0 through GDash's timing model. Fixed per level, not
  load-dependent, so games replay.
- A game second is 1000 ms of cave time (NTSC-like; PAL's was 1.2 s).
- Gameplay randomness (push, amoeba) is a seeded SplitMix64; the random fill uses
  the original C64 generator (verified against GDash's `C64RandomGenerator`).
- Quota is a plain number (no BD1 two-digit wrap). Amoeba threshold scales with
  cave size: 227 per 1000 cells, 200 in a 40x22 cave.
- Input: a key press is latched until the next tick reads it, so a tap shorter
  than a tick still moves one cell (BD1 only sampled).

## Plans (approved 2026-10-05)

**Modes**

- **Campaign**: our own caves in worlds, each world introducing its elements the
  uXo way (alone first, then paired, then mixed), with intermissions between worlds
  and five difficulty levels per cave.
- **Import**: plays cave files the player already has: BDCFF, GDash `.bd`, R'n'D
  level sets. Tens of thousands of caves on day one without shipping any of them.
- **Co-op**: two diggers in one cave (pads first), shared quota. BD never had it.
- **Race** over iroh: same cave and seed side by side, first out wins.
- **Editor**: in-terminal cave editor that saves our own text format.

**Elements beyond BD1**, own names and looks: slime and expanding wall (BD2), then
picks from the later games in the research note (section 3): keys and doors,
conveyors, gravity switch, bombs, acid, and so on. Each world brings a few.

**Terminal worlds**, only possible here, after uXo's: a cave that scrolls up like
terminal output; glitch tiles that flicker between wall and dirt; a cursor that
types new boulders into the cave.

**Cave format** (built): one text file per cave in `caves/`, documented at the top of
`src/cave.rs`: name, about, values, per-level quota, time and seed, up to four
random-fill pairs, magic and amoeba times, then the grid (`.` dirt takes the fill,
`:` dirt never does). `cargo test` checks every cave parses, is steel-walled, has
one inbox and an exit.

## Rendering

Two renderers, both kept (David, 2026-10-05):

**Pixel mode** (built, `src/pixel.rs`), after uXo's renderer: `▀` cells, two square
pixels per cell, hand-drawn art at 4, 6 and 8 pixels per tile and whole-number
upscales (12, 16, 18, 24). Riveted steel, running-bond brick (an active magic wall
glows through its mortar), shaded boulders, faceted diamonds with a glint, gritty
till, a seething amoeba, a firefly ember with a spark running round its rim, a
flapping butterfly, fire and ice blasts. Auto zoom picks the biggest size that
shows the whole cave, glyphs when none does; `+`/`-` step through glyphs and every
pixel size, `0` goes back to auto; a pixel camera eases after the hero with a dead
zone.

- At 6 pixels a whole 40x22 cave is 240x66 cells: on David's full-screen terminal
  (about 320x75) **the whole cave fits with room for the HUD, no scrolling**. At 8
  it is 320x88 and scrolls vertically; a camera with a dead zone, as in uXo.
- **Glyph mode** (built) is two cells per tile: 80x22 plus a HUD line above and a
  message line below, 80x24. A real way to play here, not only a fallback. The
  hero is a placeholder (`o/`, `\o`) until its look is designed.
- No darkness or fog of war (David's rule from uXo).

## Input

Gamepads are first-class (David's rule from uXo): stick and d-pad move, any face
button snaps; keyboard WASD or arrows, snap with Shift+direction (works in every
terminal) or Space/Enter (held with the kitty protocol, otherwise it arms one snap).
`src/input.rs` is a one-player cut of uXo's (gilrs, SDL-mapped devices only); every
pad becoming its own player comes with co-op.

## Simulation and logging

- Fixed-tick deterministic engine, seeded RNG: replays (input per tick) and iroh
  race mode come cheap, as in Tetrix and uXo.
- Session logging per the Asciiverse convention (`log` + `env_logger`): raw input
  events, state transitions, periodic state snapshot.

## Build order

1. **Engine** (done 2026-10-05): the BD1 element set, tick scan, timings;
   `cargo test` with scripted input against known behaviours; glyph renderer; one
   test cave.
2. **Look and feel**: half-block art at 4, 6, 8 (first pass done 2026-10-05), the
   hero, a pixel-font HUD, sound.
3. **First world**: 16 caves plus intermissions, difficulty levels, title and
   results screens.
4. **Import** (BDCFF, `.bd`, R'n'D) and the editor.
5. Co-op, race over iroh, later elements, terminal worlds.

## Open questions

- [x] Name: Moraen
- [x] TMview trademark search for Moraen: clear
- [x] Folder renamed `rockford` to `moraen`
- [x] David approves the proposals above (2026-10-05)
- [x] Pushed to `theasciiverse/moraen` (private, 2026-10-05)
- [ ] Our hero: look and idle animation
- [ ] Vet the 25 public-domain Boulder levels before using any
- [ ] Promote CAVEZ of PHEAR to the catalog
- [ ] BD1 engine conflicts, picked for now (see Engine decisions), David to
      confirm: amoeba timer start, hatching delay; still open: expanding-wall
      growth per tick (comes with BD2 elements)
