//! Caves: our own, one text file each in `caves/`, compiled in.
//!
//! Header lines, then `grid:` and the cave, every row the same length and walled in
//! steel all round (an exit may sit in the wall):
//!
//! ```text
//! name: First Thaw
//! about: one line for the cave card
//! value: 10 15             a diamond's worth, then past the quota
//! quota: 12 13 14 15 16    diamonds needed, difficulty levels 1 to 5
//! time: 150 140 130 120 110
//! seed: 3 41 97 150 222    the random fill's seed per level, 0 to 255
//! fill: boulder 50, diamond 8   up to four (element, chance in 256) pairs
//! magic: 20                magic wall seconds (optional)
//! amoeba: 40               seconds of slow amoeba growth (optional)
//! intermission: yes        optional; intermissions run faster
//! trolls: 30               optional; seconds left when the thaw wakes the trolls
//! grid:
//! ```
//!
//! Grid characters:
//!
//! - `W` steel (only explosions can't break it), `w` brick, `m` magic wall
//! - `.` dirt that takes the random fill, `:` dirt that never does, space is space
//! - `r` boulder, `d` diamond
//! - `f` firefly (starts facing left), `b` butterfly (starts facing down), `a` amoeba
//! - `t` a stone troll, asleep: a boulder until it wakes
//! - `P` the inbox, where the player arrives; `X` the exit

use crate::engine::{Dir, El};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tile {
    Space,
    Dirt,
    Earth,
    Brick,
    Steel,
    Magic,
    Boulder,
    Diamond,
    Firefly,
    Butterfly,
    Amoeba,
    Troll,
    Inbox,
    Exit,
}

impl Tile {
    pub fn from_char(c: char) -> Option<Tile> {
        Some(match c {
            ' ' => Tile::Space,
            '.' => Tile::Dirt,
            ':' => Tile::Earth,
            'w' => Tile::Brick,
            'W' => Tile::Steel,
            'm' => Tile::Magic,
            'r' => Tile::Boulder,
            'd' => Tile::Diamond,
            'f' => Tile::Firefly,
            'b' => Tile::Butterfly,
            'a' => Tile::Amoeba,
            't' => Tile::Troll,
            'P' => Tile::Inbox,
            'X' => Tile::Exit,
            _ => return None,
        })
    }

    fn from_name(s: &str) -> Option<Tile> {
        Some(match s {
            "space" => Tile::Space,
            "dirt" => Tile::Earth,
            "brick" => Tile::Brick,
            "steel" => Tile::Steel,
            "magic" => Tile::Magic,
            "boulder" => Tile::Boulder,
            "diamond" => Tile::Diamond,
            "firefly" => Tile::Firefly,
            "butterfly" => Tile::Butterfly,
            "amoeba" => Tile::Amoeba,
            "troll" => Tile::Troll,
            _ => return None,
        })
    }

    pub fn element(self) -> El {
        match self {
            Tile::Space => El::Space,
            Tile::Dirt | Tile::Earth => El::Dirt,
            Tile::Brick => El::Brick,
            Tile::Steel => El::Steel,
            Tile::Magic => El::Magic,
            Tile::Boulder => El::Boulder,
            Tile::Diamond => El::Diamond,
            Tile::Firefly => El::Firefly(Dir::Left),
            Tile::Butterfly => El::Butterfly(Dir::Down),
            Tile::Amoeba => El::Amoeba,
            Tile::Troll => El::TrollAsleep,
            Tile::Inbox => El::Inbox,
            Tile::Exit => El::Outbox,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Cave {
    pub name: String,
    pub about: String,
    pub value: u32,
    pub extra_value: u32,
    pub quotas: [u32; 5],
    pub times: [u32; 5],
    pub seeds: [u8; 5],
    pub fill: Vec<(Tile, u8)>,
    pub magic_secs: u32,
    pub amoeba_secs: u32,
    pub troll_secs: u32,
    pub intermission: bool,
    pub w: usize,
    pub h: usize,
    pub grid: Vec<Vec<Tile>>,
}

/// The first world, in play order: four caves, then an intermission, five times;
/// the last group brings the stone trolls.
pub const FILES: [&str; 25] = [
    include_str!("../caves/01-first-thaw.txt"),
    include_str!("../caves/02-erratic.txt"),
    include_str!("../caves/03-drumlin.txt"),
    include_str!("../caves/04-kettle.txt"),
    include_str!("../caves/05-rime.txt"),
    include_str!("../caves/06-esker.txt"),
    include_str!("../caves/07-kame.txt"),
    include_str!("../caves/08-rock-flour.txt"),
    include_str!("../caves/09-outwash.txt"),
    include_str!("../caves/10-hoarfrost.txt"),
    include_str!("../caves/11-varve.txt"),
    include_str!("../caves/12-crevasse.txt"),
    include_str!("../caves/13-nunatak.txt"),
    include_str!("../caves/14-calving.txt"),
    include_str!("../caves/15-frost-heave.txt"),
    include_str!("../caves/16-cirque.txt"),
    include_str!("../caves/17-serac.txt"),
    include_str!("../caves/18-striae.txt"),
    include_str!("../caves/19-moraine.txt"),
    include_str!("../caves/20-firn.txt"),
    include_str!("../caves/21-talus.txt"),
    include_str!("../caves/22-arete.txt"),
    include_str!("../caves/23-tarn.txt"),
    include_str!("../caves/24-bergschrund.txt"),
    include_str!("../caves/25-ablation.txt"),
];

/// Where a new game may start: the first cave after each intermission.
pub const STARTS: [usize; 5] = [0, 5, 10, 15, 20];

/// "CAVE 7" or "INTERMISSION 2": caves and intermissions are counted apart.
pub fn label(i: usize) -> String {
    let i = i % FILES.len();
    let inter = (0..=i).filter(|&j| load(j).intermission).count();
    if load(i).intermission { format!("INTERMISSION {inter}") } else { format!("CAVE {}", i + 1 - inter) }
}

pub fn load(i: usize) -> Cave {
    parse(FILES[i % FILES.len()]).unwrap_or_else(|e| panic!("cave {}: {e}", i + 1))
}

fn five<T: std::str::FromStr>(key: &str, v: &str) -> Result<[T; 5], String> {
    let parts: Vec<T> = v.split_whitespace().map(|p| p.parse().map_err(|_| format!("{key}: bad number {p:?}"))).collect::<Result<_, _>>()?;
    parts.try_into().map_err(|_| format!("{key}: needs five values, one per difficulty level"))
}

pub fn parse(text: &str) -> Result<Cave, String> {
    let mut cave = Cave {
        name: String::new(),
        about: String::new(),
        value: 0,
        extra_value: 0,
        quotas: [0; 5],
        times: [0; 5],
        seeds: [0; 5],
        fill: Vec::new(),
        magic_secs: 0,
        amoeba_secs: 0,
        troll_secs: crate::engine::TROLL_SECS,
        intermission: false,
        w: 0,
        h: 0,
        grid: Vec::new(),
    };
    let mut lines = text.lines();
    let mut seen = Vec::new();
    for line in lines.by_ref() {
        let line = line.trim_end();
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "grid:" {
            break;
        }
        let (key, v) = line.split_once(':').ok_or_else(|| format!("expected `key: value`, got {line:?}"))?;
        let v = v.trim();
        let num = |v: &str| v.parse::<u32>().map_err(|_| format!("{key}: bad number {v:?}"));
        match key {
            "name" => cave.name = v.to_string(),
            "about" => cave.about = v.to_string(),
            "value" => {
                let [a, b]: [u32; 2] = v
                    .split_whitespace()
                    .map(num)
                    .collect::<Result<Vec<_>, _>>()?
                    .try_into()
                    .map_err(|_| "value: needs two numbers, normal and extra".to_string())?;
                (cave.value, cave.extra_value) = (a, b);
            }
            "quota" => cave.quotas = five(key, v)?,
            "time" => cave.times = five(key, v)?,
            "seed" => cave.seeds = five(key, v)?,
            "fill" => {
                for pair in v.split(',') {
                    let mut it = pair.split_whitespace();
                    let (Some(name), Some(chance), None) = (it.next(), it.next(), it.next()) else {
                        return Err(format!("fill: expected `element chance`, got {pair:?}"));
                    };
                    let tile = Tile::from_name(name).ok_or_else(|| format!("fill: unknown element {name:?}"))?;
                    let chance = chance.parse::<u8>().map_err(|_| format!("fill: chance {chance:?} is not 0 to 255"))?;
                    cave.fill.push((tile, chance));
                }
                if cave.fill.len() > 4 {
                    return Err("fill: at most four pairs".into());
                }
            }
            "magic" => cave.magic_secs = num(v)?,
            "amoeba" => cave.amoeba_secs = num(v)?,
            "trolls" => cave.troll_secs = num(v)?,
            "intermission" => cave.intermission = v == "yes",
            _ => return Err(format!("unknown key {key:?}")),
        }
        seen.push(key.to_string());
    }
    for key in ["name", "value", "quota", "time", "seed"] {
        if !seen.iter().any(|k| k == key) {
            return Err(format!("missing `{key}:`"));
        }
    }
    for (y, line) in lines.enumerate() {
        if line.is_empty() {
            continue;
        }
        let row = line
            .chars()
            .enumerate()
            .map(|(x, c)| Tile::from_char(c).ok_or_else(|| format!("grid row {y}, column {x}: unknown {c:?}")))
            .collect::<Result<Vec<_>, _>>()?;
        cave.grid.push(row);
    }
    cave.h = cave.grid.len();
    cave.w = cave.grid.first().map_or(0, Vec::len);
    if cave.w < 3 || cave.h < 3 {
        return Err("grid: too small".into());
    }
    if let Some(y) = cave.grid.iter().position(|r| r.len() != cave.w) {
        return Err(format!("grid row {y} is {} wide, not {}", cave.grid[y].len(), cave.w));
    }
    let mut inboxes = 0;
    let mut exits = 0;
    for (y, row) in cave.grid.iter().enumerate() {
        for (x, &t) in row.iter().enumerate() {
            let border = x == 0 || y == 0 || x + 1 == cave.w || y + 1 == cave.h;
            if border && !matches!(t, Tile::Steel | Tile::Exit) {
                return Err(format!("grid: the border at {x},{y} must be steel (or the exit)"));
            }
            inboxes += (t == Tile::Inbox) as u32;
            exits += (t == Tile::Exit) as u32;
        }
    }
    if inboxes != 1 {
        return Err(format!("grid: needs exactly one inbox `P`, found {inboxes}"));
    }
    if exits == 0 {
        return Err("grid: needs an exit `X`".into());
    }
    Ok(cave)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cave_parses() {
        for (i, f) in FILES.iter().enumerate() {
            let c = parse(f).unwrap_or_else(|e| panic!("cave {}: {e}", i + 1));
            assert!(!c.name.is_empty());
            assert!(c.quotas.iter().all(|&q| q > 0), "{}: a quota of zero", c.name);
            assert!(c.times.iter().all(|&t| t > 0), "{}: no time", c.name);
        }
    }

    #[test]
    fn the_world_runs_four_caves_then_an_intermission() {
        for i in 0..FILES.len() {
            assert_eq!(load(i).intermission, i % 5 == 4, "{}", load(i).name);
        }
        assert_eq!(label(0), "CAVE 1");
        assert_eq!(label(4), "INTERMISSION 1");
        assert_eq!(label(5), "CAVE 5");
        assert_eq!(label(19), "INTERMISSION 4");
        for &s in &STARTS {
            assert!(!load(s).intermission);
        }
    }

    #[test]
    fn rejects_broken_caves() {
        let head = "name: t\nvalue: 1 2\nquota: 1 1 1 1 1\ntime: 9 9 9 9 9\nseed: 1 2 3 4 5\ngrid:\n";
        assert!(parse(&format!("{head}WWWW\nWP.W\nWWWW\n")).unwrap_err().contains("exit"));
        assert!(parse(&format!("{head}WWWW\nwPXW\nWWWW\n")).unwrap_err().contains("border"));
        assert!(parse(&format!("{head}WWWW\nWPXW\nWWW\n")).unwrap_err().contains("wide"));
        assert!(parse(&format!("{head}WWWWW\nWP.XW\nWWWWW\n")).is_ok());
    }
}
