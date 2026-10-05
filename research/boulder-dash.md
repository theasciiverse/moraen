# Boulder Dash (Peter Liepa / Chris Gray, First Star Software, 1984): engine research notes

Gathered 2026-10-04 for a Rust + Ratatui spiritual successor. Reference only: mechanics and
engine rules as inspiration, never names, art, cave layouts, music or cave data. Ground truth
order used below: the C64 BD1 disassembly first, then GDash source (a faithful-engine clone),
then Peter Broadribb's docs and the Inside FAQ. Where they disagree it is said so.
"Unverified" marks gaps.

Main sources:

- C64 BD1 annotated disassembly (retrointernals.org):
  `http://www.retrointernals.org/boulder-dash/boulder-dash-disassembly.html`
- GDash source, MIT (`https://github.com/meonwax/gdash`, mirror of
  `https://bitbucket.org/czirkoszoltan/gdash`): `src/cave/caverenderedengine.cpp` (the
  per-tick engine), `src/cave/elementproperties.cpp` (flags: rounded, explodes-by-hit, etc.),
  `src/cave/cavestored.cpp` (BDCFF defaults), `src/fileops/c64import.cpp` (BD1 engine
  settings and BD1 cave format), `docs/c64_internals.txt`, `docs/ckdelay.txt` (timing model)
- Peter Broadribb's BD pages (mirror): `https://www.elmerproductions.com/sp/peterb/`, esp.
  `insideBoulderdash.html`, `rockfordInfo.html`, `creatureInfo.html`, and the BDCFF object
  specs `BDCFF/objects/0000.html` to `000A.html`
- C64 Boulder Dash Inside FAQ v3.0wip, LogicDeLuxe 2006:
  `http://www.gratissaugen.de/erbsen/BD-Inside-FAQ.html`
- BDCFF 0.32 (Tim Stridmann 2004): `https://www.boulder-dash.nl/bdcff_doc.html`
- Wikipedia: Boulder Dash (video game), Supaplex, Emerald Mine, Rocks'n'Diamonds,
  Repton (video game), Boulder Dash Construction Kit
- osgameclones `games/*.yaml` (`https://github.com/opengaming/osgameclones`), GitHub search,
  crates.io search (for section 4)

## 1. Engine

### Grid

- Cave = 40 x 22 cells including a 1-cell steel (titanium) border (BDCFF default
  `CaveSize=40 22`; 880 cells). Intermissions are 20 x 12 (BDCFF `IntermissionSize`).
- C64 draws each cell as 2 x 2 characters (8 double-width px x 16 px), so a normal cave
  scrolls; an intermission is one screen. Terminal note: 40 cells x 2 columns = 80 columns,
  22 rows + a status line fits an 80 x 24 terminal with no scrolling.
- The cave is one linear 40-wide array: the right neighbour of the last column is the first
  column of the next row ("lineshift" wrap in GDash's BD1 settings). Irrelevant while the
  steel border is intact.

### The tick (one "scan")

- One game tick = one full scan of the cave: row by row, top to bottom, each row left to
  right (disassembly `ProcessCave`: x = 0..39, rows from 1 up to the row limit; intermissions
  stop earlier, so they run faster). Each cell's object code indexes a handler table.
- Objects move by writing into neighbouring cells immediately (no double buffer). So an object
  that moves right or down would be met again later in the same scan. Prevention:
  every moving object has a "scanned this tick" twin code (BD1: boulder `$10` / scanned
  `$11`, falling boulder `$12` / `$13`, diamond `$14`..`$17`, Rockford `$38` / `$39`, amoeba
  `$3A` / `$3B`, flies have 4 facing codes plus 4 scanned codes). A handler always writes the
  scanned twin when it moves something; scanned codes have no handler, and the scan converts
  them back to the base code as it passes (BD1: the cell up-left of the current one is
  normalised; GDash: unscan right after processing, plus a full pass after the scan).
  Implement as a per-cell `scanned` bit cleared at tick end; behaviour is the same.
- Consequence (Broadribb): who wins a race for a cell depends on scan order, not fairness.
  Rockford is processed when the scan reaches him, so against an object below or right of him
  he moves first; against one above or left, it moves first.
- Rockford has no special status in the loop: if he is not in the grid (before birth, after
  death) the cave simply keeps running.

### Tick timing

- BD1 (C64) has no fixed frame rate. A delay loop runs between rows (`CaveRowTransitionDelay`)
  with a per-difficulty CaveDelay: level 1 = 12, 2 = 6, 3 = 3, 4 = 1, 5 = 0 (disassembly
  `GameSpeedsForSublevelsArray = 0c 06 03 01 00`; Broadribb: 90 CPU cycles per unit). Busy
  caves (many boulders, explosions) run slower.
- GDash's measured model for BD1: tick = 88 ms + 3.66 ms x CaveDelay + per-element processing
  cost + 2.6 ms per animated type present (amoeba, firefly, butterfly), rounded to 20 ms
  (PAL fields). Cave A level 1 measured at 120 to 160 ms, average about 150 ms. Intermission
  base is 60 ms instead of 88 ms. Explosions add about 1.2 ms each.
- Construction Kit and later engines: tick = max(65 ms + element cost, 20 ms x CaveDelay),
  CaveDelay 0..20 per cave. GDash's BD2 defaults: 180 / 160 / 140 / 120 / 120 ms by level.
  BDCFF/GDash default for new caves: 200 ms.
- Practical: a fixed 120 to 200 ms tick, faster per difficulty level, reproduces the feel.
- Game seconds are separate from ticks: a raster IRQ counts 60 sub-ticks per game second
  (disassembly `SubSecondTick`, `CMP #$3c`). On PAL (50 Hz) a BD second is 1.2 real seconds
  (Inside FAQ: "only 50 Boulder Dash seconds in a real minute"; GDash `PALTiming` uses 1200 ms).
  Broadribb guessed "about 64 ticks"; the code says 60.

### Input

- The joystick is read once per tick, at the moment the scan reaches Rockford.
- 4-way only. Diagonals: the horizontal component wins (disassembly: right or any diagonal
  with right goes right; up-left / down-left go left).
- Move rules (target cell): space or dirt = move there; stationary diamond = collect and move;
  outbox (open) = exit; stationary boulder and moving left/right = push attempt; anything else
  = blocked.
- Fire + direction = "snap": the move's effect happens to the target cell (dirt dug, diamond
  collected) but Rockford stays put; the target becomes space. Fire + push moves the boulder
  one cell and leaves Rockford in place (both disassembly `MoveRockfordIfPossible` and GDash).
  Fire into empty space does nothing.

## 2. Elements and their exact rules (BD1 unless noted)

Properties summary:

| Element | Rounded (things roll off) | Explodes when hit by falling object | Destroyed by explosion | Notes |
|---|---|---|---|---|
| Space | no | no | n/a | |
| Dirt | no | no | yes | Rockford digs; amoeba grows into it |
| Brick wall | yes | no | yes | |
| Steel / titanium wall | no | no | **no** | the only explosion-proof element in BD1 |
| Magic wall | no | no | yes | looks like brick when not active |
| Boulder, stationary | yes | no | yes | |
| Boulder, falling | no | no | yes | |
| Diamond, stationary | yes | no | yes | |
| Diamond, falling | no | no | yes | |
| Firefly | no | yes, to space | yes (no chain) | |
| Butterfly | no | yes, to diamonds | yes (no chain) | |
| Rockford | no | yes, to space | yes | |
| Amoeba | no | no | yes | kills flies on contact |
| Inbox | no | no | unverified for BD1 (GDash: yes) | flashing inbox alternates with a steel look |
| Pre-Rockford stages | no | no | Broadribb: apparently yes, likely an oversight | 3 stages between inbox and Rockford |
| Outbox (closed / open) | no | no | apparently yes | closed one looks like steel |

Rounded set is verified twice: disassembly `TestIfObjectSlippery` accepts only `$10`
(stationary boulder), `$14` (stationary diamond), `$02` (brick); GDash sets `P_SLOPED` on the
same three and not on steel, magic wall, falling objects or dirt.

### Space, dirt, walls

- Dirt only holds things up: Rockford digs it (leaves space), amoeba grows into it; flies and
  falling objects cannot enter it; things do not roll off it.
- Steel: unaffected by explosions, not rounded.
- Brick: rounded, destroyed by explosions.
- BD1 also has an explosion-destructible steel lookalike (code `$06`; Inside FAQ says it was
  replaced by the hidden exit in the Construction Kit).

### Boulder and diamond (identical rules except what they turn into)

States: stationary, falling. Every scan of a boulder:

- **Stationary**: if the cell below is space, become falling and move down 1 (sound plays on
  start). Else if the cell below is rounded (stationary boulder/diamond, brick): roll if
  left AND down-left are both space (move 1 left, become falling); else if right AND
  down-right are both space, roll right. Left is always tried first. A stationary object
  sitting on a rounded one re-checks every tick, so it rolls the moment the side opens.
- **Falling**: if space below, move down 1 (still falling). Else, in this order
  (disassembly `ProcessFallingBoulder`):
  1. Magic wall below: see magic wall.
  2. Sound. If the thing below is rounded: try roll left, then right (same two-cell test);
     a falling object rolls without stopping first; if it cannot roll it becomes stationary.
  3. Rockford below (unscanned code `$38`): explosion to space centred on Rockford.
  4. Firefly below: explosion to space centred on it. Butterfly below: explosion to diamonds.
  5. Anything else: becomes stationary.
- The falling object sits in the top-middle cell of the 3 x 3 and is consumed by its own
  explosion. Amoeba is **not** killed by a falling object (the disassembly checks only
  Rockford, firefly, butterfly; one line of Broadribb's boulder spec lists amoeba, his own
  algorithm does not).
- Only a falling object kills. Rockford can stand under a stationary boulder indefinitely; it
  only falls if a space opens below it. A falling diamond kills Rockford exactly like a boulder.
- Magic wall turns a falling boulder into a diamond and a falling diamond into a boulder.

### Pushing

- Only stationary boulders, only horizontally, only if the cell beyond is space.
- Success chance per tick of trying: **1 in 4** (disassembly `RockfordTriesToMoveBoulder`:
  `AND #$03`, success on 0, using the timer-based random; GDash BD1 setting
  `pushing_stone_prob = 250000` per million). Broadribb's spec says 1 in 8; the code
  contradicts it.
- On success the boulder is written as a *scanned* stationary boulder 1 cell further, so it is
  not processed again this tick (cannot start falling in the same tick it was pushed), and
  Rockford moves into its cell (unless fire is held).

### Rockford

- Collects only **stationary** diamonds (disassembly compares the target with `$14` only;
  GDash's eat routine takes `O_DIAMOND`, not `O_DIAMOND_F`). Broadribb claims falling diamonds
  can be collected; both code sources say no.
- Diamond collected: score += current diamond value; diamonds collected += 1; if collected ==
  needed (equality), the exit-open event fires: white screen flash (flash counter 6 in
  the disassembly, Broadribb says 5 ticks), crack sound, and current diamond value switches to the
  "extra diamond" value.
- Idle animation (cosmetic): every 16-tick animation cycle, 1/4 chance to blink and 1/16
  chance to toggle foot tapping (disassembly comments confirm both).
- Death: there is no death flag. A counter of scans without seeing Rockford runs; after 16
  such scans the fire button ends the cave. Creatures, explosions and the clock keep going.

### Firefly and butterfly

- 4 facings (left, up, right, down), invisible to the player. Move only into **space**.
- Firefly each tick: if its left-hand cell (relative to facing) is space, turn left and move
  there; else if ahead is space, move ahead; else turn right **without moving**. So left turns
  are instant, a right turn costs 1 tick, a U-turn 2 ticks. This makes it a left-hand wall
  follower.
- Butterfly: mirror image (prefers right, else ahead, else turn left in place).
- Start facings in BD1 caves: fireflies face left (first move is down if possible),
  butterflies face down (first move is left if possible).
- Before moving, each fly checks its 4 orthogonal neighbours; if any is Rockford (scanned or
  not) or amoeba **that existed before this tick** (not amoeba grown this tick), the fly
  explodes on the spot. It is the fly that explodes, which kills an adjacent Rockford through
  the 3 x 3 blast. Diagonal contact is safe.
- Triggers, complete list for BD1: falling boulder/diamond landing on it; orthogonal contact
  with Rockford; orthogonal contact with amoeba; being inside another explosion (then it is
  simply consumed: no chain explosion, a butterfly caught in a blast does not add diamonds).
- Firefly blast leaves 3 x 3 space, butterfly blast leaves 3 x 3 diamonds (steel cells keep
  steel). Rockford's own blast leaves space.

### Amoeba

- Grows into space or dirt. Per amoeba cell per tick: draw a random number; grow if
  `(rand AND mask) < 4`, slow mask `$7F` (4/128, about 3.1%), fast mask `$0F` (4/16, 25%).
  The low 2 bits of the same number pick the direction (up, left, right, down); if that cell
  is space or dirt it becomes a *scanned* amoeba (does not grow until next tick).
- Slow growth switches to fast after the "amoeba slow growth time" in game seconds (cave byte
  `$01`, the same byte as the magic wall milling time; BD1 assumes they never coexist).
  When that timer starts: the disassembly increments the counter only in game-second ticks
  after the inbox phase; the Inside FAQ and GDash say BD1 starts it immediately, before
  Rockford's birth. Unresolved; unverified which is right.
- Counts are evaluated from the *previous* tick (disassembly `PreTickAmoebaProcessing`):
  - If 200 or more amoeba cells were counted last tick, every amoeba cell turns into a
    stationary boulder (Broadribb suggests scaling: 200/880 = 22.7% of the cave; BDCFF default
    `AmoebaThreshold=0.227`).
  - Else if no amoeba cell could grow last tick (enclosed), every amoeba cell turns into a
    stationary diamond.
  - "Could grow" = any orthogonal neighbour is space or dirt (BD1 tests `code < 2`). Flies,
    boulders, walls all enclose it.
- Amoeba is silent until Rockford is born. Flies explode on touching it.

### Magic wall

- Global state: dormant, active, expired. Looks and acts like brick (but not rounded) when
  dormant or expired.
- A **falling** boulder or diamond landing on any magic wall cell activates all magic wall
  for the milling time (cave byte `$01`, game seconds, counted from activation even before
  Rockford's birth). Stationary objects resting on it do nothing and do not roll off.
- Active: the object vanishes from above the wall and appears as a *falling* converted object
  2 cells down (directly below the wall), only if that cell is space; otherwise it is lost.
  It is never "inside" the wall. Expired: objects just vanish (still with sound).

### Slime (introduced in BD2 per the Inside FAQ)

- A permeable wall. A boulder or diamond resting on slime with space below it passes through
  (appears below as falling) when a random test passes. BD2 / Construction Kit use the
  predictable C64 generator: pass if `(rand AND permeability_pattern) == 0`, more set bits =
  slower; Construction Kit maps setting 0..8 to `00 10 18 38 3c 7c 7e fe ff`. GDash default
  permeability 1.0 (always passes).

### Expanding wall (BD2 per the Inside FAQ, later added to BD1 fan games)

- Looks like brick, not rounded. Each tick a piece grows into adjacent space to its left /
  right; new pieces do not grow until the next tick, so it extends 1 cell per tick.
  Broadribb's BDCFF spec grows left and right in the same tick; GDash grows left, or if that
  is blocked right (one side per tick). Grows only into space (Rockford standing there blocks
  it). Later engines: vertical and 4-way variants plus a direction switch.

### Explosions

- Always 3 x 3 centred on the exploding thing; every cell except steel is overwritten
  (disassembly `SetCellButPreserveSteelWall` checks only steel). No chain reactions in BD1.
- Stage trick: cells at or before the scan position get stage 1, cells after it get stage 0,
  so after this scan all 9 are stage 1. Explosion codes then advance 1 stage per tick
  (`$1C -> $1D -> $1E -> $1F -> space`, diamond variant `$21 -> ... -> $24 -> stationary
  diamond`): the blast is solid for 4 ticks.
- Explosion cells act as wall: creatures cannot enter, falling objects stop on them as on
  dirt (no rolling). A new blast over an old one resets it to stage 1 of the new type.

### Inbox, birth, exit

- Cave starts covered by animated steel that is removed randomly (69 rounds of one random
  cell per line, then all). Whether the cave already runs during uncovering: Broadribb says
  "I believe" so; unverified.
- Inbox flashes (alternates per scan with a steel-wall look). Countdown: 4 game seconds;
  6 if two players share one joystick; +1 on cave B (disassembly
  `InitGameVariablesFromLevelData`; Broadribb attributes the +1 to difficulty 2, which the
  code does not support). GDash's BD1 import uses 2 s hatching; unexplained difference.
- At 0: crack sound, status bar switches to the in-game display, the cave timer starts, the
  amoeba becomes audible. The inbox then morphs through 3 pre-Rockford stages, one per scan,
  into Rockford (`$25 -> $26 -> $27 -> $28 -> $38`).
- Monsters, boulders and amoeba already run during the whole inbox phase.
- Outbox starts closed (looks like steel). After the quota is met, the next time the scan
  reaches it, it becomes a flashing open outbox. Entering it ends the cave. Hidden outbox
  (invisible until open) arrived with the Construction Kit.

### Cave time, score, lives

- Cave time per difficulty level, in game seconds, counted only after birth; last 10 seconds
  play a rising ping. 0 = out of time = life lost.
- Diamond value and extra diamond value are per cave (BD1 cave A, for scale: 10 and 15).
- Exit bonus: each remaining second is counted down into the score at **difficulty level
  points per second** (1 to 5; disassembly `CaveComplete`).
- Start with 3 lives (BDCFF default; title string "3 MEN"). Extra life every 500 points and at
  the start of every intermission; maximum 9 lives.
- Diamonds-needed is stored as 2 decimal digits and checked by equality after incrementing:
  values over 99 wrap mod 100, and 0 means 100 (GDash notes and import code).

### Intermissions and cave order

- Order: A B C D, intermission 1, E F G H, int. 2, I J K L, int. 3, M N O P, int. 4. A new
  game may start at A, E, I or M on any level 1..5. After int. 4 play returns to A at the next
  difficulty level (5 stays 5). No ending.
- Intermissions: no life lost on failure, play always continues; a life is awarded on
  entering one (BD1 "instant life"; later engines can reward on completion instead).

### Difficulty levels and the random fill

- 5 levels. Per cave, each level has its own: diamonds needed, cave time, and random seed.
  Globally per level: CaveDelay (speed, above) and the time bonus multiplier. Explicit cave
  objects are identical on all levels in BD1 (later formats can show objects per level).
- So a level change re-rolls the scattered objects (same probabilities, different seed),
  tightens time and quota, and speeds the cave up.
- BD1 cave record: 32-byte header (cave no., amoeba/magic time, diamond value, extra value,
  5 seeds, 5 quotas, 5 times, 3 colours, 2 unused, 4 random object codes, 4 probabilities),
  then draw commands (point, line in 8 directions, filled rectangle, outline rectangle),
  `$FF` terminated, max 255 bytes.
- Build order: base fill dirt; for every cell of the interior rows, scan order, call the
  seeded generator once (0..255); for each of the 4 (object, probability) pairs, if
  `rand < probability` take that object, later pairs override earlier ones; then draw the
  steel border; then draw the explicit objects on top.
- Generator (Broadribb; identical in the disassembly `PseudoRandom` and GDash
  `C64RandomGenerator`), seed1 = 0 and seed2 = level seed at cave start, returns seed1:

```
t1 = (s1 & 1) << 7
t2 = (s2 >> 1) & 0x7F
r  = s2 + ((s2 & 1) << 7);  c = r >> 8;  r &= 0xFF
r  = r + c + 0x13;          c = r >> 8;  s2 = r & 0xFF
r  = s1 + c + t1;           c = r >> 8;  r &= 0xFF
s1 = (r + c + t2) & 0xFF
return s1
```

- Gameplay randomness (push, amoeba) uses a separate unseeded CIA-timer random. For replays
  and deterministic tests, seed our own PRNG instead.

### Engine quirks worth knowing

- Magic wall scan abort (BD1): a falling boulder entering a magic wall jumps to a sound routine
  whose return exits `ProcessCave`, so the rest of that tick's scan is skipped (disassembly;
  Inside FAQ describes it). Amoeba cells after that point are not counted or tested, so an
  amoeba can be judged "enclosed" and turn to diamonds. Fixed in BD2. GDash emulates it
  (`magic_wall_breakscan`).
- Inside FAQ: "A not activated Amoeba sometimes converts to Diamonds" (BD1).
- Falling objects only test for the unscanned Rockford code `$38`; a Rockford flagged as
  already processed (`$39`) is not crushed by that test (disassembly; GDash gives the scanned
  player no explodes-by-hit flag). When this matters in play: unverified.
- Flies ignore amoeba grown in the same tick, but do see a scanned Rockford.
- Rockford stall: when the screen scroll reverses direction, Rockford skips a move for one
  tick (disassembly comment; Inside FAQ). A rendering artifact, do not copy.
- Pausing does not stop the amoeba and magic wall timers (Inside FAQ).
- "Voodoo" in GDash / BDCFF is an element (the Construction Kit's "Mrs. Rockford" doll),
  not a quirk; see section 3.
- No documented "same tick push" bug beyond the scanned-boulder rule above was found.

## 3. What later games added (mechanics only)

Origin game is given where a source states it; elements known only from GDash's engine
(which implements the Construction Kit, First Boulder, Crazy Dream and Crazy Light engines)
are marked "GDash".

- Slime and horizontal expanding wall: BD2 (Inside FAQ).
- Second avatar doll ("voodoo"): Construction Kit; in later engines it catches falling
  diamonds, and its death leaves a gravestone ringed by steel plus a time penalty (Inside FAQ).
- Hidden exit: Construction Kit (Inside FAQ).
- Bladder (GDash): rises against gravity after an 8-step delay, rolls up off rounded things,
  turns into a time-bonus clock when it touches a set element.
- Bomb (GDash): pick up, place with fire + direction into space or dirt, 7-stage fuse,
  plus-shaped 5-cell blast (leaves brick by default).
- Nut (GDash): pushable and falls like a boulder; a boulder landing on it cracks it into a
  diamond.
- Ghost (GDash): random walker, explodes on touching the player or amoeba in an X pattern
  that leaves random items.
- Biter (GDash): moves on a timer, eats dirt then diamonds, spits boulders back.
- Cow (GDash): walls-following creature that becomes a skeleton (collectible) if enclosed.
- Waiting / chasing boulder (GDash): sleeps until it can fall, then homes on the player.
- Acid (GDash, Crazy Dream 3 extension per GDash import): spreads into dirt at a set chance.
- Water (GDash): floods empty space in all directions.
- Gravity switch (GDash): changes gravity direction after a delay; everything falls that way.
- Conveyor belts with on/off and direction switches (GDash): carry resting objects sideways.
- Keys 1-3 and matching doors (GDash; Emerald Mine has coloured keys per Wikipedia).
- Diamond key releasing trapped diamonds; teleporters (exit at next teleporter in scan
  order); replicator (copies the element above to below); pneumatic hammer (breaks brick);
  lava (absorbs anything entering); sweet (makes pushes always succeed); clock (+time);
  falling wall (drops when the player is straight below); nitro pack (explodes when landing
  on anything but dirt); flying boulders and diamonds (fall upward); rockets; Sokoban box
  (pushed with fire in 4 directions); creature and wall direction switches (all GDash).
- Emerald Mine (Wikipedia): two gem types (a diamond is worth 3 emeralds and a falling stone
  crushes it), an enemy that eats diamonds, killed enemies dropping 1 diamond + up to 8
  emeralds, amoebae with variants that drop lethal drips, dynamite, bombs, coloured keys and
  doors, converter walls turning stone into emerald into diamond into stone, simultaneous
  multiplayer.
- Supaplex (Wikipedia): no time limit; per-level gravity on the player; one-way "ports";
  zonks and infotrons as boulders and diamonds; scissor and spark enemies; "bugs" (dirt that
  randomly arcs lethally); orange disks (fall, explode on impact), yellow disks (pushable any
  direction, detonated by terminals), red disks (carried and dropped as timed bombs);
  sub-cell motion that creates exploitable timing tricks.
- Repton 2 (Wikipedia): spirits that follow walls on their left and turn into diamonds when
  guided into cages; Repton 3: spreading fungus, time capsules resetting the timer.
- Rocks'n'Diamonds (Wikipedia): mixes elements of all of the above plus Sokoban in one engine,
  with tape-recorder replays you can rewind and re-record from.

## 4. Existing terminal / text-mode clones (the gap)

| Project | Lang / UI | License | What it is |
|---|---|---|---|
| CAVEZ of PHEAR (`github.com/AMDmi3/cavezofphear`) | C, ncurses, 80 x 25 | GPL-3.0 | BD/Digger-like, 1 char per cell (`*` diamond, `$` money, `%` bombs, `M` monsters); bombs placed and remote-detonated; extra life per 1000 pts; packaged in distros. Tick model unverified |
| ASCII DASH (`sourceforge.net/projects/ascii-dash`) | C/C++, ncurses | MIT | "Remake of Boulder Dash for the Atari 800" in a terminal; last update 2024-07-17. Engine fidelity unverified |
| Mining Haze (`sourceforge.net/projects/mininghaze`) | C, ncurses | GPL-2.0 | BD clone; last update 2016. Details unverified |
| Wanderer (`github.com/sshipway/wanderer`) | C, curses | GPL-2.0 | old Unix text BD-like; moves are counted per screen, built-in editor, password per screen |
| nPush (`sourceforge.net/projects/npush`) | C++, ncurses | GPL-2.0 | Sokoban-like with BD elements, multiplayer (osgameclones: "similar") |
| Goulder Dash (`github.com/sergev/godash`) | Go, tcell | MIT | emoji tiles (2 columns per cell), text level files, scripted replays; interactive loop blocks on key events, so it appears turn-based (code skim, not played); port of a Scala ascii-dash |
| rad-tui (`github.com/aadhii-yz/rad-tui`) | C, raw ANSI + RGB | MIT | small tutorial-derived "rocks and diamonds" in the terminal (2026) |
| gemtunneler-tui (`github.com/akakabrian/gemtunneler-tui`) | Python, Textual | README says MIT, GitHub shows no license file | fireflies, timer, multi-cave; README is partly unfinished |

- No Ratatui or Rust terminal Boulder Dash was found (GitHub searches for "boulder dash"
  with ratatui / tui / terminal / ncurses / rust; crates.io "boulderdash" is a Bevy game,
  AGPL-3.0-only AND CC-BY-SA-4.0; `dpc/boulder-dash.rs` is Amethyst, Apache-2.0).
- As far as checked, none of the terminal clones document a BD1-faithful engine (scan order
  with scanned flags, 1-in-4 push, fly turn rules, amoeba thresholds, magic wall, timing).
  GDash is the faithful engine but is graphical. The gap: a real-time, engine-faithful,
  well-presented TUI version with original caves.

## 5. Licensing of the references

- GDash: `COPYING` is the MIT license, "Copyright (c) 2007-2013, Czirkos Zoltan" (read in the
  clone). Separate `COPYING.GTK`, `COPYING.HQX`, `COPYING.SDL` cover bundled components.
- GDash bundles cave data: `caves/` holds 81 folders, 1207 `.gds` and 31 `.bd` files, among
  them `caves/various/Boulder_Dash_1.bd`, `Boulder_Dash_2.bd`, `Boulder_Dash_3.bd`,
  `Construction_Kit.gds` (the BD1 file's header reads `Author=Peter Liepa with Chris Gray`).
  None of these files carries a license or copyright line; `AUTHORS` says only
  "Caves: Various designers." The MIT notice names only Czirkos Zoltan's copyright in "the
  Software".
- Rights holders as stated by sources: the Inside FAQ (2006) says "Boulder Dash is copyright
  by First Star Software" and "Rockford is a registered Trademark by First Star Software",
  and advises against using those names. Wikipedia: since September 2017 BBG Entertainment
  owns the Boulder Dash intellectual property. Current trademark registrations: unverified.
- BD1 disassembly page (retrointernals.org) contains the original cave data; no license
  statement was looked for. Broadribb's pages: no license text on the pages fetched.
- Rocks'n'Diamonds: code GPL-2.0-or-later; status of its game data unclear (Wikipedia citing
  Debian's copyright file).
- BDCFF is a community format spec; our own caves can use a BDCFF-like text format without
  touching any original data.

## Design notes for our engine

- Implement the engine as BD1 does it: one in-place scan per tick, per-cell scanned bit,
  rounded / explodes-by-hit / explosion-proof as element flags (GDash's property table is a
  good model), state stored in the element (stationary vs falling, fly facing).
- Seeded PRNG for both cave generation and gameplay rolls, so replays are just input logs.
- Rules whose numbers are the feel: 1-in-4 push, fly "turn costs a tick", amoeba 4/128 then
  4/16 with 200-cell cap, 4-tick explosions, horizontal-wins diagonals, 16-scan death grace.
