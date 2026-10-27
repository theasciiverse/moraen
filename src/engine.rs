//! The cave engine: the 1984 tick model, faithfully.
//!
//! One tick is one scan of the grid in place, row by row, left to right. Objects move
//! by writing into neighbouring cells at once (no double buffer); every cell written
//! this tick is marked scanned, so a thing that moved right or down is not met again
//! later in the same scan. Scan order decides races. The player's input is read once
//! per tick, when the scan reaches the player. Rules, numbers and their sources are in
//! `research/boulder-dash.md`; the quirks we chose are listed in `CLAUDE.md`.
//!
//! Deterministic: the same cave, level, seed and per-tick input give the same game, so
//! a replay is just the input list.

use crate::cave::{Cave, Tile};

/// A game second, in milliseconds of cave time (NTSC-like: a second on the clock is
/// a real second at the nominal tick rate).
pub const SECOND_MS: u32 = 1000;
/// Game seconds from the start until the inbox hatches (BD1 disassembly: 4).
pub const HATCH_SECS: u32 = 4;
/// Ticks a troll waits after a throw before it can throw again (about 1.5 s).
pub const TROLL_COOL: u8 = 12;
/// How far a troll sees the player to throw at, in cells.
pub const TROLL_REACH: usize = 12;
/// Seconds left on the clock when, by default, the thaw wakes the trolls.
pub const TROLL_SECS: u32 = 30;
/// How close (steps across and down) the player can come before a troll wakes.
pub const TROLL_SENSE: usize = 2;
/// Ticks a crushed troll's "OOF!" stays up.
pub const SHOUT_TICKS: u8 = 10;
/// Ticks without the player in the grid before a press ends the cave (BD1: 16).
pub const GRACE_TICKS: u32 = 16;
/// Tick length per difficulty level, in ms: BD1's CaveDelay 12, 6, 3, 1, 0 through
/// GDash's measured model (88 ms + 3.66 ms per delay unit + element cost, about
/// 150 ms on level 1), rounded to whole PAL fields.
pub const TICK_MS: [u32; 5] = [150, 130, 120, 110, 100];
/// Intermissions run faster: a 60 ms base instead of 88.
pub const TICK_MS_INTERMISSION: [u32; 5] = [120, 100, 90, 80, 70];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    Up,
    Right,
    Down,
    Left,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::Up, Dir::Right, Dir::Down, Dir::Left];

    /// A quarter turn anticlockwise.
    pub fn left(self) -> Dir {
        match self {
            Dir::Up => Dir::Left,
            Dir::Left => Dir::Down,
            Dir::Down => Dir::Right,
            Dir::Right => Dir::Up,
        }
    }

    /// A quarter turn clockwise.
    pub fn right(self) -> Dir {
        match self {
            Dir::Up => Dir::Right,
            Dir::Right => Dir::Down,
            Dir::Down => Dir::Left,
            Dir::Left => Dir::Up,
        }
    }

    pub fn horizontal(self) -> bool {
        matches!(self, Dir::Left | Dir::Right)
    }

    fn dxy(self) -> (i32, i32) {
        match self {
            Dir::Up => (0, -1),
            Dir::Right => (1, 0),
            Dir::Down => (0, 1),
            Dir::Left => (-1, 0),
        }
    }
}

/// What a cell holds. State lives in the element: falling or not, a fly's facing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum El {
    Space,
    Dirt,
    Brick,
    Steel,
    Magic,
    Boulder,
    BoulderFalling,
    Diamond,
    DiamondFalling,
    /// Follows the wall on its left; blows up to space.
    Firefly(Dir),
    /// Follows the wall on its right; blows up to diamonds.
    Butterfly(Dir),
    Amoeba,
    /// A stone troll asleep: a boulder in every way (it falls, rolls, is pushed and
    /// things roll off it), except that it blinks, wakes when the player comes close,
    /// never goes through a magic wall, and a falling stone kills it.
    TrollAsleep,
    TrollFalling,
    /// A stone troll awake: too heavy to climb, it walks the ground left or right
    /// every other tick, turning at anything in its way and dropping off ledges; it
    /// kills what it touches and throws stones down or sideways at the player. A falling
    /// stone kills it ("OOF!") and leaves a plain boulder; daylight from the open exit
    /// turns it back to stone.
    Troll { dir: Dir, cool: u8, rest: bool },
    /// A stone a troll threw sideways: one cell a tick; it kills the player it hits
    /// and drops as a boulder against anything else.
    Thrown(Dir),
    Inbox,
    /// The three stages between the inbox and the player, one per tick.
    Hatching(u8),
    Player,
    Outbox,
    OutboxOpen,
    /// Solid for four ticks, then space (or a diamond, or a boulder: `rubble`).
    Blast { stage: u8, diamonds: bool, rubble: bool },
}

impl El {
    /// Things roll off these: brick and resting boulders and diamonds, nothing else.
    pub fn rounded(self) -> bool {
        matches!(self, El::Brick | El::Boulder | El::Diamond | El::TrollAsleep)
    }
}

/// One tick's input for the player: a direction (diagonals already resolved,
/// horizontal wins) and whether the snap button is held.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Cmd {
    pub dir: Option<Dir>,
    pub snap: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Counting down to the player's arrival; the cave already runs.
    Hatching,
    Playing,
    /// Into the open exit. The cave stops.
    Won,
    /// The player is gone; the cave keeps running.
    Dead,
    /// The clock ran out. The cave stops.
    TimeUp,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MagicState {
    Dormant,
    Active,
    Expired,
}

/// Things that happened during a tick, for sound, effects and the log.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event {
    Hatched,
    Born,
    Dig,
    Collect,
    ExitOpened,
    Push,
    BoulderStart,
    BoulderLand,
    DiamondLand,
    Explosion,
    MagicActivated,
    MagicExpired,
    MagicPass,
    AmoebaToBoulders,
    AmoebaToDiamonds,
    /// One of the last ten seconds ticked away.
    TimePing,
    TrollWoke,
    TrollThrew,
    TrollFroze,
    TrollCrushed,
    Died,
    Won,
    TimeUp,
}

/// Gameplay randomness (pushes, amoeba). BD1 used an unseeded CIA timer; we seed a
/// SplitMix64 so games replay.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn byte(&mut self) -> u8 {
        (self.next() >> 56) as u8
    }
}

/// The C64 generator the 1984 game used to scatter a cave's random fill, seeded per
/// difficulty level (Broadribb; identical in the disassembly and GDash's
/// `C64RandomGenerator::random`, checked against GDash's source).
#[derive(Clone, Debug)]
pub struct C64Rng {
    s1: u8,
    s2: u8,
}

impl C64Rng {
    pub fn new(seed: u8) -> C64Rng {
        C64Rng { s1: 0, s2: seed }
    }

    pub fn next(&mut self) -> u8 {
        let t1 = ((self.s1 & 1) as u32) << 7;
        let t2 = ((self.s2 >> 1) & 0x7F) as u32;
        let mut r = self.s2 as u32 + (((self.s2 & 1) as u32) << 7);
        let mut c = r >> 8;
        r &= 0xFF;
        r = r + c + 0x13;
        c = r >> 8;
        self.s2 = (r & 0xFF) as u8;
        r = self.s1 as u32 + c + t1;
        c = r >> 8;
        r &= 0xFF;
        self.s1 = ((r + c + t2) & 0xFF) as u8;
        self.s1
    }
}

/// A cave in play.
#[derive(Clone, Debug)]
pub struct World {
    pub w: usize,
    pub h: usize,
    pub cells: Vec<El>,
    scanned: Vec<bool>,
    rng: Rng,
    pub tick_ms: u32,
    pub ticks: u64,
    /// Cave time not yet making up a whole game second.
    sub_ms: u32,
    pub phase: Phase,
    pub hatch_left: u32,
    pub time_left: u32,
    pub diamonds: u32,
    pub quota: u32,
    /// What the next diamond is worth (switches to the extra value at the quota).
    pub value: u32,
    pub extra_value: u32,
    pub score: u32,
    pub magic: MagicState,
    pub magic_secs: u32,
    pub magic_left: u32,
    /// Seconds of slow amoeba growth, then fast.
    pub amoeba_slow_secs: u32,
    amoeba_secs: u32,
    pub amoeba_fast: bool,
    pub amoeba_threshold: u32,
    amoeba_count_last: u32,
    amoeba_could_grow_last: bool,
    /// Where the player is, if in the grid.
    pub player: Option<usize>,
    pub facing: Dir,
    /// The player walked this tick (for the walking animation).
    pub moved: bool,
    born: bool,
    /// Ticks since the player was last seen in the grid.
    pub unseen: u32,
    /// Frames of the white flash when the exit opens.
    pub flash: u32,
    /// Seconds left on the clock when the thaw reaches the sleeping trolls.
    pub troll_wake_secs: u32,
    /// Short cries over cells, ticks left to show: a crushed troll's "OOF!".
    pub shouts: Vec<(usize, u8)>,
    pub events: Vec<Event>,
}

impl World {
    /// Build a cave at a difficulty level (1 to 5), with a seed for gameplay rolls.
    ///
    /// Like the 1984 caves: every interior cell rolls the level's C64 generator once,
    /// in scan order, and each fill pair (later ones win) whose chance beats the roll
    /// places its element. Only plain dirt (`.`) takes the fill; everything else in
    /// the grid is drawn on top, as the original's explicit objects were.
    pub fn new(cave: &Cave, level: u32, seed: u64) -> World {
        let level = level.clamp(1, 5);
        let li = (level - 1) as usize;
        let (w, h) = (cave.w, cave.h);
        let mut fill_rng = C64Rng::new(cave.seeds[li]);
        let mut cells = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let tile = cave.grid[y][x];
                let interior = y > 0 && y + 1 < h;
                let mut el = tile.element();
                if interior {
                    let r = fill_rng.next();
                    if tile == Tile::Dirt {
                        for &(fill, chance) in &cave.fill {
                            if r < chance {
                                el = fill.element();
                            }
                        }
                    }
                }
                cells.push(el);
            }
        }
        let n = cells.len();
        let tick_ms = if cave.intermission { TICK_MS_INTERMISSION[li] } else { TICK_MS[li] };
        World {
            w,
            h,
            cells,
            scanned: vec![false; n],
            rng: Rng::new(seed ^ ((level as u64) << 56)),
            tick_ms,
            ticks: 0,
            sub_ms: 0,
            phase: Phase::Hatching,
            hatch_left: HATCH_SECS,
            time_left: cave.times[li],
            diamonds: 0,
            quota: cave.quotas[li],
            value: cave.value,
            extra_value: cave.extra_value,
            score: 0,
            magic: MagicState::Dormant,
            magic_secs: cave.magic_secs,
            magic_left: 0,
            amoeba_slow_secs: cave.amoeba_secs,
            amoeba_secs: 0,
            amoeba_fast: false,
            // BD1: 200 cells of a 40x22 cave; scaled for other sizes (BDCFF's 0.227)
            amoeba_threshold: ((n as u32) * 227 + 500) / 1000,
            amoeba_count_last: 0,
            amoeba_could_grow_last: true,
            player: None,
            facing: Dir::Right,
            moved: false,
            born: false,
            unseen: 0,
            flash: 0,
            troll_wake_secs: cave.troll_secs,
            shouts: Vec::new(),
            events: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn at(&self, x: usize, y: usize) -> El {
        self.cells[y * self.w + x]
    }

    #[cfg(test)]
    pub fn set(&mut self, x: usize, y: usize, el: El) {
        self.cells[y * self.w + x] = el;
    }

    #[cfg(test)]
    pub fn dir_right_for_test(&mut self, x: usize, y: usize) {
        if let El::Troll { cool, rest, .. } = self.cells[y * self.w + x] {
            self.cells[y * self.w + x] = El::Troll { dir: Dir::Right, cool, rest };
        }
    }

    pub fn exit_open(&self) -> bool {
        self.diamonds >= self.quota
    }

    /// The cell one step from `i`, if it is inside the cave. Off-grid reads as steel.
    fn step(&self, i: usize, d: Dir) -> Option<usize> {
        let (dx, dy) = d.dxy();
        let x = (i % self.w) as i32 + dx;
        let y = (i / self.w) as i32 + dy;
        (x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h).then(|| y as usize * self.w + x as usize)
    }

    fn get(&self, i: Option<usize>) -> El {
        i.map_or(El::Steel, |i| self.cells[i])
    }

    /// Write `el` at `i` and mark it scanned, so it is not processed again this tick.
    fn put(&mut self, i: usize, el: El) {
        self.cells[i] = el;
        self.scanned[i] = true;
    }

    /// Run one tick with this input for the player.
    pub fn tick(&mut self, cmd: Cmd) {
        self.events.clear();
        if matches!(self.phase, Phase::Won | Phase::TimeUp) {
            return;
        }
        self.flash = self.flash.saturating_sub(1);
        self.shouts.retain_mut(|(_, t)| {
            *t -= 1;
            *t > 0
        });
        self.moved = false;
        // the amoeba's fate is decided from last tick's count
        let amoeba_to = if self.amoeba_count_last >= self.amoeba_threshold {
            Some(El::Boulder)
        } else if self.amoeba_count_last > 0 && !self.amoeba_could_grow_last {
            Some(El::Diamond)
        } else {
            None
        };
        match amoeba_to {
            Some(El::Boulder) => self.events.push(Event::AmoebaToBoulders),
            Some(_) => self.events.push(Event::AmoebaToDiamonds),
            None => {}
        }
        let mut amoeba = Amoeba { to: amoeba_to, count: 0, could_grow: false };
        let mut seen = false;

        for i in 0..self.cells.len() {
            if self.scanned[i] {
                continue;
            }
            match self.cells[i] {
                El::Player => {
                    seen = true;
                    self.player(i, cmd);
                }
                El::Boulder => self.object(i, Obj::Boulder, false),
                El::BoulderFalling => self.object(i, Obj::Boulder, true),
                El::Diamond => self.object(i, Obj::Diamond, false),
                El::DiamondFalling => self.object(i, Obj::Diamond, true),
                El::Firefly(d) => self.fly(i, d, false),
                El::Butterfly(d) => self.fly(i, d, true),
                El::TrollAsleep => {
                    if self.troll_wakes(i) {
                        self.cells[i] = El::Troll { dir: Dir::Left, cool: TROLL_COOL / 2, rest: true };
                        self.events.push(Event::TrollWoke);
                    } else {
                        self.object(i, Obj::Troll, false);
                    }
                }
                El::TrollFalling => self.object(i, Obj::Troll, true),
                El::Troll { dir, cool, rest } => self.troll(i, dir, cool, rest),
                El::Thrown(d) => self.thrown(i, d),
                El::Amoeba => self.amoeba(i, &mut amoeba),
                El::Blast { stage, diamonds, rubble } => {
                    self.cells[i] = if stage >= 3 {
                        if diamonds {
                            El::Diamond
                        } else if rubble {
                            El::Boulder
                        } else {
                            El::Space
                        }
                    } else {
                        El::Blast { stage: stage + 1, diamonds, rubble }
                    };
                }
                El::Inbox if self.phase != Phase::Hatching => self.cells[i] = El::Hatching(1),
                El::Hatching(n) => {
                    if n >= 3 {
                        self.cells[i] = El::Player;
                        self.player = Some(i);
                        self.born = true;
                        seen = true;
                        self.events.push(Event::Born);
                    } else {
                        self.cells[i] = El::Hatching(n + 1);
                    }
                }
                El::Outbox if self.exit_open() => self.cells[i] = El::OutboxOpen,
                _ => {}
            }
        }
        self.scanned.fill(false);
        self.amoeba_count_last = amoeba.count;
        self.amoeba_could_grow_last = amoeba.could_grow;

        if seen {
            self.unseen = 0;
        } else {
            self.unseen += 1;
            if self.born {
                self.player = None;
                if self.phase == Phase::Playing {
                    self.phase = Phase::Dead;
                    self.events.push(Event::Died);
                }
            }
        }

        self.sub_ms += self.tick_ms;
        while self.sub_ms >= SECOND_MS {
            self.sub_ms -= SECOND_MS;
            self.second();
        }
        self.ticks += 1;
    }

    /// One game second passed.
    fn second(&mut self) {
        match self.phase {
            Phase::Hatching => {
                self.hatch_left = self.hatch_left.saturating_sub(1);
                if self.hatch_left == 0 {
                    self.phase = Phase::Playing;
                    self.events.push(Event::Hatched);
                }
            }
            Phase::Playing => {
                self.time_left = self.time_left.saturating_sub(1);
                if self.time_left == 0 {
                    self.phase = Phase::TimeUp;
                    self.events.push(Event::TimeUp);
                } else if self.time_left <= 10 {
                    self.events.push(Event::TimePing);
                }
            }
            _ => {}
        }
        // the magic wall's time runs from activation, even before the player arrives
        if self.magic == MagicState::Active {
            self.magic_left = self.magic_left.saturating_sub(1);
            if self.magic_left == 0 {
                self.magic = MagicState::Expired;
                self.events.push(Event::MagicExpired);
            }
        }
        // the amoeba's slow time runs once the inbox has hatched (the disassembly)
        if self.phase != Phase::Hatching && !self.amoeba_fast {
            self.amoeba_secs += 1;
            if self.amoeba_secs >= self.amoeba_slow_secs {
                self.amoeba_fast = true;
            }
        }
    }

    fn player(&mut self, i: usize, cmd: Cmd) {
        self.player = Some(i);
        let Some(d) = cmd.dir else { return };
        if d.horizontal() {
            self.facing = d;
        }
        let Some(t) = self.step(i, d) else { return };
        let snap = cmd.snap;
        match self.cells[t] {
            El::Space => {
                // snapping at empty space does nothing
                if !snap {
                    self.walk(i, t);
                }
            }
            El::Dirt => {
                self.events.push(Event::Dig);
                if snap { self.cells[t] = El::Space } else { self.walk(i, t) }
            }
            // only a resting diamond can be picked up
            El::Diamond => {
                self.collect();
                if snap { self.cells[t] = El::Space } else { self.walk(i, t) }
            }
            El::OutboxOpen if !snap => {
                self.walk(i, t);
                self.phase = Phase::Won;
                self.events.push(Event::Won);
            }
            // a sleeping troll is pushed like any boulder
            stone @ (El::Boulder | El::TrollAsleep) if d.horizontal() => {
                let beyond = self.step(t, d);
                // a push works one tick in four (disassembly: AND #$03)
                if self.get(beyond) == El::Space && self.rng.byte() & 3 == 0 {
                    let b = beyond.expect("space is in the grid");
                    self.put(b, stone);
                    self.events.push(Event::Push);
                    if snap { self.cells[t] = El::Space } else { self.walk(i, t) }
                }
            }
            _ => {}
        }
    }

    fn walk(&mut self, from: usize, to: usize) {
        self.cells[from] = El::Space;
        self.put(to, El::Player);
        self.player = Some(to);
        self.moved = true;
    }

    fn collect(&mut self) {
        self.diamonds += 1;
        self.score += self.value;
        self.events.push(Event::Collect);
        if self.diamonds == self.quota {
            self.value = self.extra_value;
            self.flash = 6;
            self.events.push(Event::ExitOpened);
        }
    }

    /// A boulder or a diamond, resting or falling.
    fn object(&mut self, i: usize, obj: Obj, falling: bool) {
        let below = self.step(i, Dir::Down);
        let under = self.get(below);
        if under == El::Space {
            let b = below.expect("space is in the grid");
            self.cells[i] = El::Space;
            self.put(b, obj.falling());
            if !falling && obj == Obj::Boulder {
                self.events.push(Event::BoulderStart);
            }
            return;
        }
        if falling {
            // a troll is no gem: it lands on a magic wall as on rock
            if under == El::Magic && obj != Obj::Troll {
                self.through_magic(i, obj);
                return;
            }
            self.events.push(if obj == Obj::Boulder { Event::BoulderLand } else { Event::DiamondLand });
        }
        // a falling stone kills a troll, asleep or awake: it's a plain boulder now
        if falling && matches!(under, El::TrollAsleep | El::Troll { .. }) {
            let b = below.expect("a troll is in the grid");
            self.put(b, El::Boulder);
            self.cells[i] = obj.resting();
            self.shouts.push((b, SHOUT_TICKS));
            self.events.push(Event::TrollCrushed);
            return;
        }
        if under.rounded() {
            if !self.roll(i, obj) {
                self.cells[i] = obj.resting();
            }
            return;
        }
        if !falling {
            return;
        }
        let b = below.expect("a crushable thing is in the grid");
        match under {
            // only the player not yet moved this tick (BD1 tests the unscanned code)
            El::Player if !self.scanned[b] => self.explode(b, false, i),
            El::Firefly(_) => self.explode(b, false, i),
            El::Butterfly(_) => self.explode(b, true, i),
            _ => self.cells[i] = obj.resting(),
        }
    }

    /// Roll off something rounded: left first, needing the side and the cell below
    /// it both empty. Rolling turns the object falling.
    fn roll(&mut self, i: usize, obj: Obj) -> bool {
        for d in [Dir::Left, Dir::Right] {
            let Some(side) = self.step(i, d) else { continue };
            if self.cells[side] == El::Space && self.get(self.step(side, Dir::Down)) == El::Space {
                self.cells[i] = El::Space;
                self.put(side, obj.falling());
                return true;
            }
        }
        false
    }

    /// A falling object hits a magic wall: the first hit wakes every magic wall; while
    /// active the object comes out two cells down, converted and still falling, if
    /// there is room; otherwise (or once expired) it is lost. The 1984 game also cut the
    /// rest of that tick's scan short here; we don't (BD2 fixed it too).
    fn through_magic(&mut self, i: usize, obj: Obj) {
        if self.magic == MagicState::Dormant {
            self.magic = MagicState::Active;
            self.magic_left = self.magic_secs;
            self.events.push(Event::MagicActivated);
        }
        self.cells[i] = El::Space;
        self.events.push(Event::MagicPass);
        if self.magic == MagicState::Active {
            let out = self.step(i, Dir::Down).and_then(|b| self.step(b, Dir::Down));
            if let Some(o) = out.filter(|&o| self.cells[o] == El::Space) {
                self.put(o, obj.other().falling());
            }
        }
    }

    fn fly(&mut self, i: usize, d: Dir, butterfly: bool) {
        // touching the player (moved or not) or older amoeba blows the fly up
        for n in Dir::ALL {
            let Some(c) = self.step(i, n) else { continue };
            match self.cells[c] {
                El::Player => return self.explode(i, butterfly, i),
                El::Amoeba if !self.scanned[c] => return self.explode(i, butterfly, i),
                _ => {}
            }
        }
        let make = |d| if butterfly { El::Butterfly(d) } else { El::Firefly(d) };
        // fireflies keep a wall on their left, butterflies on their right: the
        // preferred turn is free, the other costs a tick
        let (prefer, otherwise) = if butterfly { (d.right(), d.left()) } else { (d.left(), d.right()) };
        for go in [prefer, d] {
            if let Some(t) = self.step(i, go).filter(|&t| self.cells[t] == El::Space) {
                self.cells[i] = El::Space;
                self.put(t, make(go));
                return;
            }
        }
        self.cells[i] = make(otherwise);
    }

    /// Whether a sleeping troll wakes this tick: the player within `TROLL_SENSE`
    /// cells, or the thaw (the clock down to the cave's troll time). Never once the
    /// exit is open.
    fn troll_wakes(&self, i: usize) -> bool {
        if self.phase != Phase::Playing || self.exit_open() {
            return false;
        }
        self.time_left <= self.troll_wake_secs || self.player_within(i, TROLL_SENSE)
    }

    /// Whether the player is within `n` steps (across and down) of cell `i`.
    pub fn player_within(&self, i: usize, n: usize) -> bool {
        self.player.is_some_and(|p| {
            let (px, py, x, y) = (p % self.w, p / self.w, i % self.w, i / self.w);
            px.abs_diff(x) + py.abs_diff(y) <= n
        })
    }

    fn troll(&mut self, i: usize, d: Dir, cool: u8, rest: bool) {
        // daylight through the open exit turns it back to stone
        if self.exit_open() {
            self.cells[i] = El::TrollAsleep;
            self.events.push(Event::TrollFroze);
            return;
        }
        // its touch kills, and the blast leaves rubble
        if Dir::ALL.iter().any(|&n| self.get(self.step(i, n)) == El::Player) {
            return self.explode_into(i, Leaves::Rubble, i);
        }
        let cool = cool.saturating_sub(1);
        if let Some((d_throw, at)) = (cool == 0).then(|| self.troll_aim(i)).flatten() {
            self.cells[i] = El::Troll { dir: d, cool: TROLL_COOL, rest };
            self.put(at, if d_throw == Dir::Down { El::BoulderFalling } else { El::Thrown(d_throw) });
            self.events.push(Event::TrollThrew);
            return;
        }
        if rest {
            self.cells[i] = El::Troll { dir: d, cool, rest: false };
            return;
        }
        // too heavy to climb: with nothing under it, it drops; on the ground it walks
        // on, and turns round at anything in its way (that costs its step)
        let fall = self.step(i, Dir::Down).filter(|&t| self.cells[t] == El::Space);
        let walk = if d.horizontal() { d } else { Dir::Left };
        let ahead = self.step(i, walk).filter(|&t| self.cells[t] == El::Space);
        match fall.or(ahead) {
            Some(t) => {
                self.cells[i] = El::Space;
                self.put(t, El::Troll { dir: walk, cool, rest: true });
            }
            None => self.cells[i] = El::Troll { dir: walk.left().left(), cool, rest: true },
        }
    }

    /// Where a troll throws, if the player is straight below it or level with it,
    /// with nothing but space between (up to `TROLL_REACH` cells): the direction and
    /// the cell the stone starts in.
    fn troll_aim(&self, i: usize) -> Option<(Dir, usize)> {
        for d in [Dir::Down, Dir::Left, Dir::Right] {
            let first = self.step(i, d)?;
            let mut c = first;
            for _ in 0..TROLL_REACH {
                match self.cells[c] {
                    El::Player if c != first => return Some((d, first)),
                    El::Space => {}
                    _ => break,
                }
                match self.step(c, d) {
                    Some(n) => c = n,
                    None => break,
                }
            }
        }
        None
    }

    /// A thrown stone: on into space, into the player it hits, or down as a boulder.
    fn thrown(&mut self, i: usize, d: Dir) {
        let Some(t) = self.step(i, d) else {
            self.cells[i] = El::BoulderFalling;
            return;
        };
        match self.cells[t] {
            El::Space => {
                self.cells[i] = El::Space;
                self.put(t, El::Thrown(d));
            }
            El::Player => {
                self.cells[i] = El::Space;
                self.explode(t, false, i);
            }
            _ => self.cells[i] = El::BoulderFalling,
        }
    }

    fn amoeba(&mut self, i: usize, a: &mut Amoeba) {
        if let Some(to) = a.to {
            self.cells[i] = to;
            return;
        }
        a.count += 1;
        if Dir::ALL.iter().any(|&n| matches!(self.get(self.step(i, n)), El::Space | El::Dirt)) {
            a.could_grow = true;
        }
        // grow if (rand AND mask) < 4: 4 in 128 slow, 4 in 16 fast; the low two bits
        // pick up, left, right or down
        let r = self.rng.byte();
        let mask = if self.amoeba_fast { 0x0F } else { 0x7F };
        if r & mask < 4 {
            let d = [Dir::Up, Dir::Left, Dir::Right, Dir::Down][(r & 3) as usize];
            if let Some(t) = self.step(i, d).filter(|&t| matches!(self.cells[t], El::Space | El::Dirt)) {
                self.put(t, El::Amoeba);
            }
        }
    }

    /// A 3x3 blast centred on `c`; everything but steel is overwritten. Cells the
    /// scan has not reached yet are marked scanned, so all nine stand at the same
    /// stage after this tick, as in the original. No chain reactions.
    fn explode(&mut self, c: usize, diamonds: bool, scan: usize) {
        self.explode_into(c, if diamonds { Leaves::Diamonds } else { Leaves::Space }, scan);
    }

    /// A blast that leaves space, diamonds or (a troll's) boulders.
    fn explode_into(&mut self, c: usize, leaves: Leaves, scan: usize) {
        let (diamonds, rubble) = (leaves == Leaves::Diamonds, leaves == Leaves::Rubble);
        let (cx, cy) = ((c % self.w) as i32, (c / self.w) as i32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (x, y) = (cx + dx, cy + dy);
                if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
                    continue;
                }
                let n = y as usize * self.w + x as usize;
                if self.cells[n] == El::Steel {
                    continue;
                }
                self.cells[n] = El::Blast { stage: 1, diamonds, rubble };
                if n > scan {
                    self.scanned[n] = true;
                }
            }
        }
        self.events.push(Event::Explosion);
    }

    /// Whether the player has been gone long enough for a press to end the cave.
    pub fn can_retry(&self) -> bool {
        self.phase == Phase::TimeUp || (self.phase == Phase::Dead && self.unseen >= GRACE_TICKS)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Leaves {
    Space,
    Diamonds,
    Rubble,
}

struct Amoeba {
    to: Option<El>,
    count: u32,
    could_grow: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Obj {
    Boulder,
    Diamond,
    Troll,
}

impl Obj {
    fn resting(self) -> El {
        match self {
            Obj::Boulder => El::Boulder,
            Obj::Diamond => El::Diamond,
            Obj::Troll => El::TrollAsleep,
        }
    }
    fn falling(self) -> El {
        match self {
            Obj::Boulder => El::BoulderFalling,
            Obj::Diamond => El::DiamondFalling,
            Obj::Troll => El::TrollFalling,
        }
    }
    fn other(self) -> Obj {
        match self {
            Obj::Boulder => Obj::Diamond,
            Obj::Diamond => Obj::Boulder,
            // trolls never reach a magic wall's far side (see `object`)
            Obj::Troll => Obj::Troll,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cave;

    const NONE: Cmd = Cmd { dir: None, snap: false };

    fn go(d: Dir) -> Cmd {
        Cmd { dir: Some(d), snap: false }
    }

    fn snap(d: Dir) -> Cmd {
        Cmd { dir: Some(d), snap: true }
    }

    /// A test cave from a grid, with no random fill. `P` is the player, already
    /// born and playing (the inbox is skipped); without a `P` the cave has none.
    fn world(grid: &str) -> World {
        world_seeded(grid, 7)
    }

    fn world_seeded(grid: &str, seed: u64) -> World {
        let grid: Vec<Vec<cave::Tile>> = grid
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| l.chars().map(|c| cave::Tile::from_char(c).unwrap()).collect())
            .collect();
        let c = cave::Cave {
            name: "test".into(),
            about: String::new(),
            value: 10,
            extra_value: 20,
            quotas: [2; 5],
            times: [100; 5],
            seeds: [0; 5],
            fill: Vec::new(),
            magic_secs: 3,
            amoeba_secs: 1000,
            // no thaw unless a test sets one
            troll_secs: 0,
            intermission: false,
            w: grid[0].len(),
            h: grid.len(),
            grid,
        };
        let mut w = World::new(&c, 1, seed);
        if let Some(inbox) = w.cells.iter().position(|&e| e == El::Inbox) {
            w.cells[inbox] = El::Player;
            w.player = Some(inbox);
            w.born = true;
        }
        w.phase = Phase::Playing;
        w
    }

    fn find(w: &World, el: El) -> Option<(usize, usize)> {
        w.cells.iter().position(|&e| e == el).map(|i| (i % w.w, i / w.w))
    }

    #[test]
    fn c64_generator_matches_a_hand_run() {
        // seed1 = 0, seed2 = 0 worked by hand through the published algorithm:
        // r = 0 + 0 = 0, c 0; r = 0 + 0 + 0x13 = 0x13, s2 = 0x13; s1 = 0 + 0 + 0 = 0,
        // then 0 + 0 + t2 (0) = 0. Second call: t1 0, t2 = 0x13 >> 1 = 9;
        // r = 0x13 + 0x80 = 0x93, c 0; r = 0x93 + 0x13 = 0xA6; s1 = 0 + 0 + 0 + 9 = 9
        let mut r = C64Rng::new(0);
        assert_eq!(r.next(), 0);
        assert_eq!(r.next(), 9);
    }

    #[test]
    fn a_boulder_falls_one_cell_per_tick() {
        let mut w = world("WWWWW\nW r W\nW   W\nW   W\nWWWWW");
        w.tick(NONE);
        assert_eq!(w.at(2, 2), El::BoulderFalling);
        w.tick(NONE);
        assert_eq!(w.at(2, 3), El::BoulderFalling);
        w.tick(NONE);
        assert_eq!(w.at(2, 3), El::Boulder, "lands on steel and rests");
    }

    #[test]
    fn a_falling_boulder_moves_once_per_scan_not_down_the_whole_column() {
        // the scanned mark stops it being met again further down the same scan
        let mut w = world("WWW\nWrW\nW W\nW W\nW W\nWWW");
        w.tick(NONE);
        assert_eq!(w.at(1, 2), El::BoulderFalling);
        assert_eq!(w.at(1, 3), El::Space);
    }

    #[test]
    fn dirt_holds_a_boulder_up() {
        let mut w = world("WWWWW\nW r W\nW . W\nWWWWW");
        for _ in 0..5 {
            w.tick(NONE);
        }
        assert_eq!(w.at(2, 1), El::Boulder);
    }

    #[test]
    fn rolling_off_rounded_things_tries_left_first() {
        let mut w = world("WWWWWWW\nW  r  W\nW  w  W\nW     W\nWWWWWWW");
        w.tick(NONE);
        assert_eq!(w.at(2, 1), El::BoulderFalling, "rolled left");
        // blocked on the left: rolls right
        let mut w = world("WWWWWWW\nW  r  W\nW .w  W\nW     W\nWWWWWWW");
        w.tick(NONE);
        assert_eq!(w.at(4, 1), El::BoulderFalling, "rolled right");
        // both sides blocked below: stays
        let mut w = world("WWWWWWW\nW  r  W\nW .w. W\nW     W\nWWWWWWW");
        w.tick(NONE);
        assert_eq!(w.at(3, 1), El::Boulder);
    }

    #[test]
    fn nothing_rolls_off_steel_or_dirt() {
        let mut w = world("WWWWWWW\nW  r  W\nW  W  W\nW     W\nWWWWWWW");
        w.tick(NONE);
        assert_eq!(w.at(3, 1), El::Boulder);
    }

    #[test]
    fn a_falling_boulder_kills_the_player_a_resting_one_does_not() {
        let mut w = world("WWWWW\nW r W\nW P W\nW   W\nWWWWW");
        for _ in 0..30 {
            w.tick(NONE);
        }
        assert_eq!(w.phase, Phase::Playing, "a boulder resting on the player's head is harmless");
        let mut w = world("WWWWW\nW r W\nW   W\nW P W\nWWWWW");
        w.tick(NONE);
        w.tick(NONE);
        assert!(matches!(w.at(2, 3), El::Blast { diamonds: false, .. }));
        w.tick(NONE);
        assert_eq!(w.phase, Phase::Dead);
    }

    #[test]
    fn blasts_last_four_ticks_and_spare_steel() {
        let mut w = world("WWWWWWW\nW  r  W\nW     W\nW  f  W\nWWWWWWW");
        // a firefly boxed in under a boulder that is about to fall
        w.set(3, 3, El::Firefly(Dir::Left));
        w.set(2, 3, El::Brick);
        w.set(4, 3, El::Brick);
        w.tick(NONE); // boulder starts falling into row 2
        w.tick(NONE); // lands on the firefly: blast
        assert!(matches!(w.at(3, 3), El::Blast { .. }));
        assert_eq!(w.at(3, 4), El::Steel, "steel survives");
        assert!(matches!(w.at(2, 3), El::Blast { .. }), "brick does not");
        let mut n = 0;
        while matches!(w.at(3, 3), El::Blast { .. }) {
            w.tick(NONE);
            n += 1;
        }
        assert_eq!(n, 3, "solid on the tick it went off and three more (stage 1d..1f)");
        assert_eq!(w.at(3, 3), El::Space);
    }

    #[test]
    fn a_butterfly_crushed_leaves_diamonds() {
        let mut w = world("WWWWWWW\nW  r  W\nW     W\nWw b wW\nWwwwwwW\nWWWWWWW");
        w.set(3, 3, El::Butterfly(Dir::Down));
        w.set(2, 3, El::Brick);
        w.set(4, 3, El::Brick);
        w.tick(NONE);
        w.tick(NONE);
        for _ in 0..4 {
            w.tick(NONE);
        }
        let diamonds = w.cells.iter().filter(|&&e| matches!(e, El::Diamond | El::DiamondFalling)).count();
        assert_eq!(diamonds, 9);
    }

    #[test]
    fn the_player_digs_and_collects_only_resting_diamonds() {
        let mut w = world("WWWWWW\nWP.d W\nWWWWWW");
        w.tick(go(Dir::Right));
        assert_eq!(find(&w, El::Player), Some((2, 1)));
        w.tick(go(Dir::Right));
        assert_eq!(w.diamonds, 1);
        assert_eq!(w.score, 10);
        // a falling diamond is a wall to the player
        let mut w = world("WWWWW\nWPd W\nW   W\nWWWWW");
        w.set(2, 1, El::DiamondFalling);
        w.tick(go(Dir::Right));
        assert_eq!(w.diamonds, 0);
    }

    #[test]
    fn snapping_takes_without_moving() {
        let mut w = world("WWWWWW\nWP.d W\nWWWWWW");
        w.tick(snap(Dir::Right));
        assert_eq!(find(&w, El::Player), Some((1, 1)));
        assert_eq!(w.at(2, 1), El::Space);
        w.tick(go(Dir::Right));
        w.tick(snap(Dir::Right));
        assert_eq!(find(&w, El::Player), Some((2, 1)));
        assert_eq!(w.diamonds, 1);
    }

    #[test]
    fn the_quota_opens_the_exit_and_raises_the_value() {
        let mut w = world("WWWWWWW\nWPddd X\nWWWWWWW");
        w.tick(go(Dir::Right));
        w.tick(go(Dir::Right));
        assert!(w.events.contains(&Event::ExitOpened));
        assert!(w.flash > 0);
        w.tick(go(Dir::Right));
        assert_eq!(w.score, 10 + 10 + 20, "the third diamond is worth the extra value");
        assert_eq!(w.at(6, 1), El::OutboxOpen, "the exit opened when the scan reached it");
        w.tick(go(Dir::Right));
        w.tick(go(Dir::Right));
        assert_eq!(w.phase, Phase::Won);
    }

    #[test]
    fn pushes_work_one_try_in_four() {
        let mut ok = 0;
        let tries = 4000;
        for seed in 0..tries {
            let mut w = world_seeded("WWWWWW\nWPr  W\nWWWWWW", seed);
            w.tick(go(Dir::Right));
            if w.at(3, 1) == El::Boulder {
                ok += 1;
                assert_eq!(find(&w, El::Player), Some((2, 1)));
            }
        }
        let rate = ok as f64 / tries as f64;
        assert!((0.22..0.28).contains(&rate), "push rate {rate}");
    }

    #[test]
    fn a_pushed_boulder_does_not_fall_in_the_same_tick() {
        // a hole beyond: the boulder lands over it scanned, and falls next tick
        let mut seed = 0;
        loop {
            let mut w = world_seeded("WWWWWW\nWPr  W\nWWW WW\nWWWWWW", seed);
            w.tick(go(Dir::Right));
            if w.at(3, 1) == El::Boulder {
                w.tick(NONE);
                assert_eq!(w.at(3, 2), El::BoulderFalling);
                break;
            }
            seed += 1;
        }
    }

    #[test]
    fn snap_push_moves_the_boulder_but_not_the_player() {
        let mut seed = 0;
        loop {
            let mut w = world_seeded("WWWWWW\nWPr  W\nWWWWWW", seed);
            w.tick(snap(Dir::Right));
            if w.at(3, 1) == El::Boulder {
                assert_eq!(find(&w, El::Player), Some((1, 1)));
                assert_eq!(w.at(2, 1), El::Space);
                break;
            }
            seed += 1;
        }
    }

    #[test]
    fn boulders_cannot_be_pushed_up_or_down() {
        for seed in 0..50 {
            let mut w = world_seeded("WWW\nW W\nWrW\nWPW\nWWW", seed);
            w.tick(go(Dir::Up));
            assert_eq!(w.at(1, 2), El::Boulder);
        }
    }

    #[test]
    fn a_firefly_turns_left_for_free_and_right_at_a_cost() {
        // facing left in a corridor, wall below: goes ahead
        let mut w = world("WWWWWW\nW   fW\nWWWWWW");
        w.tick(NONE);
        assert_eq!(w.at(3, 1), El::Firefly(Dir::Left));
        // facing left with an opening below (its left): turns and moves in one tick
        let mut w = world("WWWWW\nW  fW\nWWW W\nWWWWW");
        w.tick(NONE);
        assert_eq!(w.at(3, 2), El::Firefly(Dir::Down));
        // dead end: turns right on the spot, moving nothing
        let mut w = world("WWWW\nWWfW\nWWWW\nWWWW");
        w.tick(NONE);
        assert_eq!(w.at(2, 1), El::Firefly(Dir::Up));
    }

    #[test]
    fn a_butterfly_is_the_mirror_image() {
        // facing down, its right is west: an opening there is taken at once
        let mut w = world("WWWWW\nW bWW\nWWWWW");
        w.set(2, 1, El::Butterfly(Dir::Down));
        w.tick(NONE);
        assert_eq!(w.at(1, 1), El::Butterfly(Dir::Left));
    }

    #[test]
    fn a_firefly_circles_its_ring_forever() {
        let mut w = world("WWWWWWW\nW     W\nW www W\nW     W\nWWWWWWW");
        w.set(1, 1, El::Firefly(Dir::Left));
        let mut seen = std::collections::HashSet::new();
        for _ in 0..60 {
            w.tick(NONE);
            seen.insert(w.cells.iter().position(|e| matches!(e, El::Firefly(_))).unwrap());
        }
        assert_eq!(seen.len(), 12, "visits every corridor cell");
    }

    #[test]
    fn touching_the_player_blows_the_fly_up_and_kills() {
        let mut w = world("WWWWWW\nWP f W\nWWWWWW");
        w.set(3, 1, El::Firefly(Dir::Left));
        w.tick(NONE); // fly moves next to the player
        w.tick(NONE); // touches: explodes
        assert!(w.events.contains(&Event::Explosion));
        w.tick(NONE);
        assert_eq!(w.phase, Phase::Dead);
    }

    #[test]
    fn diagonal_contact_is_safe() {
        let mut w = world("WWWWW\nWPWWW\nWWfWW\nWWWWW");
        w.set(2, 2, El::Firefly(Dir::Left));
        for _ in 0..10 {
            w.tick(NONE);
        }
        assert_eq!(w.phase, Phase::Playing);
    }

    #[test]
    fn enclosed_amoeba_turns_to_diamonds() {
        let mut w = world("WWWWW\nWwwwW\nWwawW\nWwwwW\nWWWWW");
        w.tick(NONE); // counted: enclosed
        w.tick(NONE); // converted
        assert_eq!(w.at(2, 2), El::Diamond);
        assert!(w.events.contains(&Event::AmoebaToDiamonds));
    }

    #[test]
    fn amoeba_grows_and_turns_to_boulders_at_the_threshold() {
        let mut w = world("WWWWWWWWWW\nWa       W\nW        W\nW        W\nWWWWWWWWWW");
        w.amoeba_fast = true;
        // 50 cells: the threshold scales to 11 (200 of 880 in a full cave)
        assert_eq!(w.amoeba_threshold, 11);
        let mut boulders = false;
        for _ in 0..500 {
            w.tick(NONE);
            if w.events.contains(&Event::AmoebaToBoulders) {
                boulders = true;
                break;
            }
        }
        assert!(boulders);
        assert!(!w.cells.contains(&El::Amoeba));
    }

    #[test]
    fn a_falling_boulder_does_not_kill_amoeba() {
        let mut w = world("WWWWW\nW r W\nW   W\nWwawW\nWwwwW\nWWWWW");
        w.tick(NONE);
        w.tick(NONE);
        assert!(!w.events.contains(&Event::Explosion));
    }

    #[test]
    fn magic_wall_turns_boulders_to_diamonds_until_it_expires() {
        let mut w = world("WWWWW\nW r W\nW   W\nWwmwW\nW   W\nW   W\nWWWWW");
        w.tick(NONE); // falls to row 2
        w.tick(NONE); // hits the wall: comes out two down as a falling diamond
        assert_eq!(w.magic, MagicState::Active);
        assert_eq!(w.at(2, 4), El::DiamondFalling);
        assert_eq!(w.at(2, 2), El::Space);
        // after three seconds it's spent, and things just vanish into it
        while w.magic == MagicState::Active {
            w.tick(NONE);
        }
        w.set(2, 2, El::BoulderFalling);
        w.tick(NONE);
        assert_eq!(w.at(2, 2), El::Space);
        assert_eq!(w.cells.iter().filter(|&&e| e == El::Boulder || e == El::BoulderFalling).count(), 0);
    }

    #[test]
    fn a_resting_boulder_on_magic_wall_does_nothing() {
        let mut w = world("WWWWW\nW r W\nWwmwW\nW   W\nWWWWW");
        for _ in 0..5 {
            w.tick(NONE);
        }
        assert_eq!(w.magic, MagicState::Dormant);
        assert_eq!(w.at(2, 1), El::Boulder);
    }

    #[test]
    fn scan_order_decides_races() {
        // the boulder, earlier in the scan, falls into the cell before the player
        // below-right of it can walk there
        let mut w = world("WWWWW\nW r W\nW  PW\nW...W\nWWWWW");
        w.tick(go(Dir::Left));
        assert_eq!(w.at(2, 2), El::BoulderFalling);
        assert_eq!(find(&w, El::Player), Some((3, 2)));
        // left alone, this boulder rolls off the brick into the gap at (2,2) ...
        let race = "WWWWW\nW   W\nWP rW\nWW wW\nWWWWW";
        let mut w = world(race);
        w.tick(NONE);
        assert_eq!(w.at(2, 2), El::BoulderFalling);
        // ... but the player, earlier in the scan, gets there first
        let mut w = world(race);
        w.tick(go(Dir::Right));
        assert_eq!(find(&w, El::Player), Some((2, 2)));
        assert_eq!(w.at(3, 2), El::Boulder);
    }

    #[test]
    fn the_inbox_hatches_after_four_seconds_then_three_stages() {
        let c = cave::parse("name: t\nvalue: 1 2\nquota: 1 1 1 1 1\ntime: 9 9 9 9 9\nseed: 0 0 0 0 0\ngrid:\nWWWWW\nWP dX\nWWWWW\n").unwrap();
        let mut w = World::new(&c, 1, 1);
        let mut t = 0;
        while w.phase == Phase::Hatching {
            w.tick(NONE);
            t += 1;
        }
        assert_eq!(t, (HATCH_SECS * SECOND_MS).div_ceil(TICK_MS[0]) as i32);
        assert_eq!(w.player, None);
        for n in 1..=3 {
            w.tick(NONE);
            assert_eq!(w.at(1, 1), El::Hatching(n));
        }
        w.tick(NONE);
        assert_eq!(w.at(1, 1), El::Player);
        assert!(w.events.contains(&Event::Born));
    }

    #[test]
    fn the_clock_runs_out() {
        let mut w = world("WWWWW\nWP  W\nWWWWW");
        w.time_left = 2;
        let mut pings = 0;
        while w.phase == Phase::Playing {
            w.tick(NONE);
            pings += w.events.iter().filter(|&&e| e == Event::TimePing).count();
        }
        assert_eq!(w.phase, Phase::TimeUp);
        assert_eq!(pings, 1);
        assert!(w.can_retry());
    }

    #[test]
    fn a_death_needs_sixteen_ticks_before_a_retry() {
        let mut w = world("WWWWW\nW r W\nW   W\nW P W\nWWWWW");
        while w.phase != Phase::Dead {
            w.tick(NONE);
        }
        let mut n = 0;
        while !w.can_retry() {
            w.tick(NONE);
            n += 1;
        }
        assert_eq!(n, GRACE_TICKS - 1);
    }

    #[test]
    fn same_seed_same_game() {
        let c = cave::load(0);
        let run = |seed| {
            let mut w = World::new(&c, 3, seed);
            for t in 0..400u32 {
                let d = [Dir::Right, Dir::Down, Dir::Left, Dir::Down][(t / 7 % 4) as usize];
                w.tick(go(d));
            }
            w.cells.clone()
        };
        assert_eq!(run(42), run(42));
    }

    #[test]
    fn the_random_fill_only_lands_on_plain_dirt() {
        let c = cave::load(0);
        for level in 1..=5 {
            let w = World::new(&c, level, 0);
            for y in 0..c.h {
                for x in 0..c.w {
                    if c.grid[y][x] != cave::Tile::Dirt {
                        assert_eq!(w.at(x, y), c.grid[y][x].element(), "level {level} at {x},{y}");
                    }
                }
            }
        }
    }

    /// Diamonds a cave can give at a level, roughly: loose ones, nine per
    /// butterfly, a boulder for every one above a magic wall, and the cells of each
    /// amoeba pocket small enough to be enclosed.
    fn diamonds_available(w: &World) -> u32 {
        let mut n = 0;
        let mut magic_cols = vec![false; w.w];
        for (i, &e) in w.cells.iter().enumerate() {
            match e {
                El::Diamond => n += 1,
                El::Butterfly(_) => n += 9,
                El::Magic => magic_cols[i % w.w] = true,
                _ => {}
            }
        }
        if w.magic_secs > 0 {
            for (i, &e) in w.cells.iter().enumerate() {
                let (x, y) = (i % w.w, i / w.w);
                if e == El::Boulder && magic_cols[x] && (y + 1..w.h).any(|yy| w.at(x, yy) == El::Magic) {
                    n += 1;
                }
            }
        }
        let mut seen = vec![false; w.cells.len()];
        for start in 0..w.cells.len() {
            if w.cells[start] != El::Amoeba || seen[start] {
                continue;
            }
            let mut stack = vec![start];
            let mut size = 0;
            seen[start] = true;
            while let Some(i) = stack.pop() {
                size += 1;
                for d in Dir::ALL {
                    if let Some(j) = w.step(i, d).filter(|&j| !seen[j] && matches!(w.cells[j], El::Space | El::Dirt | El::Amoeba)) {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
            if size < w.amoeba_threshold {
                n += size;
            }
        }
        n
    }

    /// Whether the exit can be reached from the inbox at all: anything but steel,
    /// brick and magic wall counts as passable (boulders move, flies die).
    fn exit_reachable(w: &World) -> bool {
        let start = w.cells.iter().position(|&e| e == El::Inbox).unwrap();
        let mut seen = vec![false; w.cells.len()];
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            if w.cells[i] == El::Outbox {
                return true;
            }
            for d in Dir::ALL {
                if let Some(j) = w.step(i, d).filter(|&j| !seen[j] && !matches!(w.cells[j], El::Steel | El::Brick | El::Magic)) {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        false
    }

    #[test]
    fn every_cave_is_playable_at_every_level() {
        let mut problems = Vec::new();
        for i in 0..cave::FILES.len() {
            let c = cave::load(i);
            let mut line = format!("{:2} {:12}", i + 1, c.name);
            for level in 1..=5 {
                let w = World::new(&c, level, 0);
                let have = diamonds_available(&w);
                let need = w.quota;
                line += &format!(" L{level} {need:>2}/{have:<3}");
                // slack: a quarter more than the quota, so one lost butterfly or a
                // crushed diamond doesn't end the cave
                if have * 4 < need * 5 {
                    problems.push(format!("{} level {level}: {have} diamonds for a quota of {need}", c.name));
                }
                if !exit_reachable(&w) {
                    problems.push(format!("{} level {level}: the exit is walled off", c.name));
                }
                let inbox = w.cells.iter().position(|&e| e == El::Inbox).unwrap();
                let (ix, iy) = ((inbox % w.w) as i32, (inbox / w.w) as i32);
                for (j, &e) in w.cells.iter().enumerate() {
                    let (x, y) = ((j % w.w) as i32, (j / w.w) as i32);
                    if matches!(e, El::Firefly(_) | El::Butterfly(_)) && (x - ix).abs() + (y - iy).abs() <= 4 {
                        problems.push(format!("{} level {level}: a fly starts next to the inbox", c.name));
                    }
                    // the first dig must not wake one
                    if e == El::TrollAsleep && (x - ix).abs() + (y - iy).abs() <= 4 {
                        problems.push(format!("{} level {level}: a troll sleeps next to the inbox", c.name));
                    }
                }
            }
            println!("{line}");
        }
        assert!(problems.is_empty(), "\n{}", problems.join("\n"));
    }

    fn awake(cool: u8, rest: bool) -> El {
        El::Troll { dir: Dir::Left, cool, rest }
    }

    fn is_troll(e: El) -> bool {
        matches!(e, El::Troll { .. })
    }

    #[test]
    fn a_sleeping_troll_falls_and_lands_like_a_boulder() {
        let mut w = world("WWWWW\nW t W\nW   W\nW   W\nWWWWW");
        w.tick(NONE);
        assert_eq!(w.at(2, 2), El::TrollFalling);
        w.tick(NONE);
        assert_eq!(w.at(2, 3), El::TrollFalling);
        w.tick(NONE);
        assert_eq!(w.at(2, 3), El::TrollAsleep, "rests on the floor");
    }

    #[test]
    fn a_boulder_rolls_off_a_sleeping_troll() {
        let mut w = world("WWWWW\nW r W\nW t W\nWWWWW");
        w.tick(NONE);
        assert_eq!(w.at(2, 1), El::Space);
        assert_eq!(w.at(1, 1), El::BoulderFalling);
        assert_eq!(w.at(2, 2), El::TrollAsleep);
    }

    #[test]
    fn the_thaw_wakes_the_trolls() {
        let mut w = world("WWWWW\nW t W\nWWWWW");
        w.tick(NONE);
        assert_eq!(w.at(2, 1), El::TrollAsleep, "no thaw yet");
        w.troll_wake_secs = w.time_left;
        w.tick(NONE);
        assert!(is_troll(w.at(2, 1)));
    }

    #[test]
    fn an_awake_troll_walks_every_other_tick() {
        let mut w = world("WWWWWWWWW\nW       W\nWWWWWWWWW");
        w.set(6, 1, awake(200, false));
        for _ in 0..4 {
            w.tick(NONE);
        }
        assert!(is_troll(w.at(4, 1)), "two cells in four ticks: {:?}", w.cells);
    }

    #[test]
    fn an_awake_troll_cannot_climb_but_drops_off_a_ledge() {
        // a slot with space above: it stays on its floor
        let mut w = world("WWWWW\nW:::W\nW: :W\nW: :W\nWWWWW");
        w.set(2, 3, awake(200, false));
        for _ in 0..8 {
            w.tick(NONE);
        }
        assert!(is_troll(w.at(2, 3)), "it never climbs");
        // off the end of a ledge it falls
        let mut w = world("WWWWWW\nW    W\nWw   W\nW    W\nWWWWWW");
        w.set(1, 1, awake(200, false));
        w.dir_right_for_test(1, 1);
        for _ in 0..10 {
            w.tick(NONE);
        }
        assert!(w.cells.iter().enumerate().any(|(i, &e)| is_troll(e) && i / w.w == 3), "it reached the floor");
    }

    #[test]
    fn a_troll_throws_down_at_the_player_below() {
        let mut w = world("WWWWW\nW   W\nW   W\nW   W\nW P W\nWWWWW");
        w.set(2, 1, awake(1, true));
        w.tick(NONE);
        assert!(w.events.contains(&Event::TrollThrew));
        assert_eq!(w.at(2, 2), El::BoulderFalling);
        for _ in 0..20 {
            w.tick(NONE);
        }
        assert_eq!(w.phase, Phase::Dead);
    }

    #[test]
    fn a_troll_throws_sideways_and_the_stone_kills() {
        let mut w = world("WWWWWWWW\nW     PW\nWWWWWWWW");
        w.set(1, 1, awake(1, true));
        w.tick(NONE);
        assert_eq!(w.at(2, 1), El::Thrown(Dir::Right));
        for _ in 0..30 {
            w.tick(NONE);
        }
        assert_eq!(w.phase, Phase::Dead);
    }

    #[test]
    fn a_thrown_stone_drops_as_a_boulder_against_a_wall() {
        let mut w = world("WWWWWWWW\nW     PW\nWWWWWWWW");
        w.set(1, 1, awake(1, true));
        w.tick(NONE);
        w.set(4, 1, El::Brick);
        for _ in 0..4 {
            w.tick(NONE);
        }
        assert!(matches!(w.at(3, 1), El::Boulder | El::BoulderFalling), "{:?}", w.at(3, 1));
        assert_eq!(w.phase, Phase::Playing);
    }

    #[test]
    fn a_trolls_touch_kills_and_leaves_rubble() {
        let mut w = world("WWWWWW\nW P  W\nWWWWWW");
        w.set(3, 1, awake(200, true));
        w.tick(NONE);
        assert!(w.cells.iter().any(|e| matches!(e, El::Blast { rubble: true, .. })));
        for _ in 0..20 {
            w.tick(NONE);
        }
        assert_eq!(w.phase, Phase::Dead);
    }

    #[test]
    fn daylight_from_the_open_exit_turns_trolls_back_to_stone() {
        let mut w = world("WWWWWWW\nW     W\nWWWWWWW");
        w.set(3, 1, awake(200, false));
        w.diamonds = w.quota;
        w.tick(NONE);
        assert!(w.events.contains(&Event::TrollFroze));
        assert!(find(&w, El::TrollAsleep).is_some());
        // and the thaw no longer wakes them
        w.troll_wake_secs = 1000;
        w.tick(NONE);
        assert!(!w.cells.iter().any(|&e| is_troll(e)));
    }

    #[test]
    fn a_troll_lands_on_a_magic_wall_as_on_rock() {
        let mut w = world("WWWWW\nW t W\nW   W\nW m W\nW   W\nWWWWW");
        for _ in 0..6 {
            w.tick(NONE);
        }
        assert_eq!(w.at(2, 2), El::TrollAsleep);
        assert_eq!(w.magic, MagicState::Dormant);
        assert_eq!(w.at(2, 4), El::Space);
    }




    #[test]
    fn coming_close_wakes_a_sleeping_troll() {
        let mut w = world("WWWWWWWW\nWP   t W\nWWWWWWWW");
        w.tick(NONE);
        assert_eq!(w.at(5, 1), El::TrollAsleep, "four away: asleep");
        w.tick(go(Dir::Right));
        w.tick(NONE);
        assert_eq!(w.at(5, 1), El::TrollAsleep, "three away: still asleep");
        w.tick(go(Dir::Right));
        w.tick(NONE);
        assert!(is_troll(w.at(5, 1)), "two away: awake");
    }

    #[test]
    fn a_falling_stone_kills_a_troll_and_leaves_a_boulder() {
        for troll in [El::TrollAsleep, awake(200, true)] {
            let mut w = world("WWWWW\nW r W\nW   W\nWW WW\nWWWWW");
            w.set(2, 3, troll);
            let mut crushed = false;
            for _ in 0..6 {
                w.tick(NONE);
                crushed |= w.events.contains(&Event::TrollCrushed);
                if crushed {
                    assert_eq!(w.shouts.len(), 1, "OOF!");
                }
            }
            assert!(crushed, "{troll:?}");
            assert_eq!(w.at(2, 3), El::Boulder);
            assert!(!w.cells.iter().any(|e| matches!(e, El::Blast { .. })), "no blast");
        }
    }

    #[test]
    fn the_caves_perch_crushes_a_caged_troll_from_safety() {
        // brick ledge, troll, space, dirt, stone; dirt cages its sides
        let mut w = world("WWWWWWW\nW  :  W\nW  r  W\nW P:  W\nW : : W\nW :t: W\nW  w  W\nWWWWWWW");
        w.tick(snap(Dir::Right));
        for _ in 0..6 {
            w.tick(NONE);
        }
        assert_eq!(w.at(3, 5), El::Boulder, "the troll is a plain boulder now");
        assert_eq!(w.phase, Phase::Playing, "and the digger lives");
    }

}
