//! Pixel mode, after uXo's renderer: half-blocks (`▀`, fg = top pixel, bg = bottom),
//! so a terminal cell is two roughly square pixels. A tile is `px` pixels square:
//! hand-drawn art at 4, 6 and 8, and whole-number upscales of the 6 and 8 art above
//! that (12, 16, 18, 24), so art pixels never come out uneven.
//!
//! All art is ours. The hero is a placeholder until its look is designed.

use crate::engine::{Dir, El, MagicState, World};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Tile size in pixels; 0 is glyph mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Zoom(pub i32);

const SIZES: [i32; 8] = [0, 4, 6, 8, 12, 16, 18, 24];

impl Zoom {
    pub const GLYPH: Zoom = Zoom(0);

    pub fn px(self) -> i32 {
        self.0
    }

    /// Terminal cells per tile.
    pub fn cells(self) -> (usize, usize) {
        if self.0 == 0 { (2, 1) } else { (self.0 as usize, self.0 as usize / 2) }
    }

    pub fn step(self, by: i32) -> Zoom {
        let i = SIZES.iter().position(|&s| s == self.0).unwrap_or(0) as i32;
        Zoom(SIZES[(i + by).clamp(0, SIZES.len() as i32 - 1) as usize])
    }

    /// A size from the command line: a pixel count, or `g` for glyphs.
    pub fn parse(s: &str) -> Option<Zoom> {
        if s == "g" {
            return Some(Zoom::GLYPH);
        }
        s.parse().ok().filter(|v| SIZES.contains(v)).map(Zoom)
    }

    pub fn name(self) -> String {
        if self.0 == 0 { "glyphs".into() } else { format!("{}px", self.0) }
    }

    /// The biggest pixel size that shows the whole cave in a `w` x `h` cell view;
    /// glyphs when none does (they are the smallest: 2x1 cells a tile).
    pub fn fit(cave_w: usize, cave_h: usize, w: usize, h: usize) -> Zoom {
        SIZES
            .iter()
            .rev()
            .filter(|&&s| s > 0)
            .map(|&s| Zoom(s))
            .find(|z| {
                let (cw, ch) = z.cells();
                cave_w * cw <= w && cave_h * ch <= h
            })
            .unwrap_or(Zoom::GLYPH)
    }

    /// The biggest pixel size that shows the whole cave; 4 px (scrolling) when none
    /// does.
    pub fn fit_pixels(cave_w: usize, cave_h: usize, w: usize, h: usize) -> Zoom {
        match Zoom::fit(cave_w, cave_h, w, h) {
            Zoom::GLYPH => Zoom(4),
            z => z,
        }
    }

    /// Which hand-drawn art set, and how many times it is scaled up.
    fn art(self) -> (i32, i32) {
        match self.0 {
            s if s % 8 == 0 => (8, s / 8),
            s if s % 6 == 0 => (6, s / 6),
            s => (4, s / 4),
        }
    }
}

type Rgb = [u8; 3];

const SPACE: Rgb = [8, 7, 10];

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    std::array::from_fn(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t.clamp(0.0, 1.0)) as u8)
}

/// Cheap per-pixel noise, the same every frame for the same inputs.
fn hash(a: i32, b: i32, c: u64) -> u32 {
    let mut h = (a as u32).wrapping_mul(0x9e37_79b1) ^ (b as u32).wrapping_mul(0x85eb_ca77) ^ (c as u32).wrapping_mul(0xc2b2_ae3d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^ (h >> 12)
}

/// A pixel buffer looking at the cave: drawing takes cave pixel coordinates, and
/// whatever falls outside the window at (`ox`, `oy`) is dropped.
struct Canvas {
    w: i32,
    h: i32,
    ox: i32,
    oy: i32,
    px: Vec<Rgb>,
}

impl Canvas {
    fn set(&mut self, x: i32, y: i32, c: Rgb) {
        let (x, y) = (x - self.ox, y - self.oy);
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.px[(y * self.w + x) as usize] = c;
        }
    }

    fn block(&mut self, x: i32, y: i32, k: i32, c: Rgb) {
        for yy in 0..k {
            for xx in 0..k {
                self.set(x + xx, y + yy, c);
            }
        }
    }

    /// Stamp art: one char per art pixel, each drawn `k` x `k`; '.' is transparent.
    fn sprite(&mut self, x: i32, y: i32, k: i32, rows: &[&str], flip: bool, pal: impl Fn(char) -> Option<Rgb>) {
        for (v, row) in rows.iter().enumerate() {
            let n = row.chars().count() as i32;
            for (u, ch) in row.chars().enumerate() {
                let u = if flip { n - 1 - u as i32 } else { u as i32 };
                if let Some(c) = pal(ch) {
                    self.block(x + u * k, y + v as i32 * k, k, c);
                }
            }
        }
    }
}

// ---- the art -------------------------------------------------------------------

/// Till: packed earth with grit and the odd pebble, fixed per tile.
fn dirt(cv: &mut Canvas, x0: i32, y0: i32, a: i32, k: i32, tx: usize, ty: usize) {
    for v in 0..a {
        for u in 0..a {
            let h = hash(u + tx as i32 * a, v + ty as i32 * a, 3) % 16;
            let c = match h {
                0..=6 => [104, 70, 40],
                7..=11 => [92, 61, 34],
                12 | 13 => [126, 88, 52],
                14 => [72, 47, 26],
                _ => [146, 128, 104],
            };
            cv.block(x0 + u * k, y0 + v * k, k, c);
        }
    }
}

fn steel(a: i32) -> &'static [&'static str] {
    match a {
        8 => &["lllllllm", "lmmmmmmd", "lmrmmrmd", "lmmmmmmd", "lmmmmmmd", "lmrmmrmd", "lmmmmmmd", "mddddddd"],
        6 => &["llllld", "lrmmrd", "lmmmmd", "lmmmmd", "lrmmrd", "dddddd"],
        _ => &["lllm", "lrmd", "lmmd", "mddd"],
    }
}

fn steel_pal(lit: f32) -> impl Fn(char) -> Option<Rgb> {
    move |c| {
        let base = match c {
            'l' => [168, 174, 188],
            'm' => [112, 118, 134],
            'd' => [62, 66, 80],
            'r' => [200, 206, 220],
            _ => return None,
        };
        Some(mix(base, [255, 255, 255], lit))
    }
}

fn brick(a: i32) -> &'static [&'static str] {
    match a {
        8 => &["lllllmll", "bbbbbmbb", "bbbbbmbb", "mmmmmmmm", "lmllllll", "bmbbbbbb", "bmbbbbbb", "mmmmmmmm"],
        6 => &["llllml", "bbbbmb", "mmmmmm", "lmllll", "bmbbbb", "mmmmmm"],
        _ => &["llml", "mmmm", "mlll", "mmmm"],
    }
}

/// Brick colours; an active magic wall glows through its mortar and shimmers.
fn brick_pal(magic: Option<u64>) -> impl Fn(char) -> Option<Rgb> {
    move |c| {
        let (l, b, m) = ([186, 92, 64], [150, 66, 44], [58, 44, 40]);
        Some(match (c, magic) {
            ('l', None) => l,
            ('b', None) => b,
            ('m', None) => m,
            ('m', Some(t)) => [[255, 90, 210], [130, 150, 255], [90, 255, 220], [255, 230, 120]][(t / 2 % 4) as usize],
            ('l', Some(_)) => [200, 110, 150],
            ('b', Some(_)) => [150, 70, 120],
            _ => return None,
        })
    }
}

fn boulder(a: i32) -> &'static [&'static str] {
    match a {
        8 => &["..dddd..", ".dlhmmd.", "dlhmmmmd", "dlmmmmsd", "dmmmmmsd", "dmmmmssd", ".dsssdd.", "..dddd.."],
        6 => &[".dddd.", "dlhmmd", "dlmmsd", "dmmmsd", "dmsssd", ".dddd."],
        _ => &[".dd.", "dhmd", "dmsd", ".dd."],
    }
}

fn boulder_pal(c: char) -> Option<Rgb> {
    Some(match c {
        'h' => [226, 218, 198],
        'l' => [190, 180, 160],
        'm' => [150, 140, 122],
        's' => [108, 100, 88],
        'd' => [62, 56, 50],
        _ => return None,
    })
}

/// Troll stone: the boulder's shades with a touch of moss.
fn troll_pal(c: char) -> Option<Rgb> {
    Some(match c {
        'h' => [206, 214, 186],
        'l' => [170, 180, 150],
        'm' => [130, 140, 114],
        's' => [92, 102, 80],
        'd' => [52, 58, 44],
        _ => return None,
    })
}

/// A troll's face on its boulder: sleepy slits while blinking, or awake, glowing
/// amber eyes under a heavy brow and a scowl (the eyes glancing now and then).
fn troll_face(cv: &mut Canvas, x0: i32, y0: i32, a: i32, k: i32, awake: bool, t: u64) {
    let (ey, ex) = match a {
        8 => (3, [2, 5]),
        6 => (2, [1, 4]),
        _ => (1, [1, 2]),
    };
    let eye: Rgb = if awake { [255, 176, 40] } else { [40, 44, 34] };
    let glance = if awake && t % 6 == 0 { 1 } else { 0 };
    for x in ex {
        cv.block(x0 + (x + glance).min(a - 2) * k, y0 + ey * k, k, eye);
    }
    if awake && a >= 6 {
        let brow: Rgb = [44, 48, 36];
        for x in ex {
            cv.block(x0 + x * k, y0 + (ey - 1) * k, k, brow);
        }
        let my = if a == 8 { 5 } else { 4 };
        for x in ex[0]..=ex[1] {
            cv.block(x0 + x * k, y0 + my * k, k, brow);
        }
    }
}

fn diamond(a: i32) -> &'static [&'static str] {
    match a {
        8 => &["........", "..hhhh..", ".hlllcc.", "hhlllccd", ".llccdd.", "..lcdd..", "...cd...", "........"],
        6 => &["......", ".hhhc.", "hllccd", ".lccd.", "..cd..", "......"],
        _ => &[".hc.", "hlcd", ".cd.", "...."],
    }
}

/// Diamond colours, with a glint that runs across now and then.
fn diamond_pal(glint: bool) -> impl Fn(char) -> Option<Rgb> {
    move |c| {
        Some(match c {
            'h' if glint => [255, 255, 255],
            'h' => [200, 250, 255],
            'l' if glint => [200, 250, 255],
            'l' => [110, 225, 255],
            'c' => [40, 160, 225],
            'd' => [22, 92, 168],
            _ => return None,
        })
    }
}

/// The hero, a placeholder: a hooded digger with a pick, facing left or right.
fn hero(a: i32, step: bool) -> Vec<&'static str> {
    match a {
        8 => vec!["..hhh...", ".hhhhh..", ".hsese.p", "..sss..p", ".bbbbbbp", "..bbb...", "..b.b...", if step { ".f...f.." } else { "..f.f..." }],
        6 => vec![".hhh..", "hsese.", ".sss.p", "bbbbbp", ".b.b..", if step { "f...f." } else { ".f.f.." }],
        _ => vec![".hh.", ".se.", "bbbp", if step { "f..f" } else { ".ff." }],
    }
}

fn hero_pal(c: char) -> Option<Rgb> {
    Some(match c {
        'h' => [255, 176, 0],
        's' => [240, 200, 160],
        'e' => [30, 24, 20],
        'b' => [200, 120, 20],
        'f' => [110, 70, 30],
        'p' => [200, 200, 210],
        _ => return None,
    })
}

/// A firefly: a square ember whose bright spark runs round its rim.
fn firefly(cv: &mut Canvas, x0: i32, y0: i32, a: i32, k: i32, t: u64) {
    let m = if a >= 6 { 1 } else { 0 };
    let n = a - 2 * m;
    let rim = 4 * (n - 1);
    let spark = (t % rim as u64) as i32;
    for v in 0..n {
        for u in 0..n {
            let edge = u == 0 || v == 0 || u == n - 1 || v == n - 1;
            let col = if edge {
                // position along the rim, clockwise from the top-left
                let p = if v == 0 {
                    u
                } else if u == n - 1 {
                    n - 1 + v
                } else if v == n - 1 {
                    3 * (n - 1) - u
                } else {
                    4 * (n - 1) - v
                };
                let d = (p - spark).rem_euclid(rim);
                if d == 0 {
                    [255, 248, 190]
                } else if d >= rim - 2 {
                    [255, 190, 80]
                } else {
                    [235, 110, 30]
                }
            } else if (u == n / 2 || u == (n - 1) / 2) && (v == n / 2 || v == (n - 1) / 2) {
                [255, 150, 40]
            } else {
                [96, 30, 14]
            };
            cv.block(x0 + (u + m) * k, y0 + (v + m) * k, k, col);
        }
    }
}

/// A butterfly, wings open, closing, shut.
fn butterfly(a: i32, frame: u64) -> &'static [&'static str] {
    match (a, frame % 4) {
        (8, 0) | (8, 2) => &["w......w", "ww....ww", "wsw..wsw", "wwwbbwww", "wwwbbwww", "wsw..wsw", "ww....ww", "w......w"],
        (8, 1) => &["........", ".w....w.", ".ww..ww.", ".wsbbsw.", ".wwbbww.", ".ww..ww.", ".w....w.", "........"],
        (8, _) => &["........", "...ww...", "..wwww..", "..wbbw..", "..wbbw..", "..wwww..", "...ww...", "........"],
        (6, 0) | (6, 2) => &["w....w", "ww..ww", "wsbbsw", "wwbbww", "ww..ww", "w....w"],
        (6, 1) => &["......", ".w..w.", ".wbbw.", ".wbbw.", ".w..w.", "......"],
        (6, _) => &["......", "..ww..", ".wbbw.", ".wbbw.", "..ww..", "......"],
        (_, 0) | (_, 2) => &["w..w", "wbbw", "wbbw", "w..w"],
        _ => &[".ww.", ".bb.", ".bb.", ".ww."],
    }
}

fn butterfly_pal(c: char) -> Option<Rgb> {
    Some(match c {
        'w' => [200, 90, 255],
        's' => [255, 200, 255],
        'b' => [70, 30, 90],
        _ => return None,
    })
}

/// Amoeba: a seething green mass, its pattern crawling.
fn amoeba(cv: &mut Canvas, x0: i32, y0: i32, a: i32, k: i32, tx: usize, ty: usize, t: u64) {
    for v in 0..a {
        for u in 0..a {
            let h = hash(u + tx as i32 * a, v + ty as i32 * a, t / 3) % 10;
            let c = match h {
                0..=3 => [40, 140, 50],
                4..=6 => [60, 180, 60],
                7 | 8 => [28, 100, 36],
                _ => [150, 240, 120],
            };
            cv.block(x0 + u * k, y0 + v * k, k, c);
        }
    }
}

/// A blast cell: white-hot, then fire, then embers; or a cold flash that sets into
/// a diamond.
#[allow(clippy::too_many_arguments)]
fn blast(cv: &mut Canvas, x0: i32, y0: i32, a: i32, k: i32, tx: usize, ty: usize, stage: u8, diamonds: bool, rubble: bool, t: u64) {
    let hot: [Rgb; 4] = if rubble {
        // a troll bursting: stone dust settling into boulders
        [[240, 236, 220], [200, 194, 178], [150, 144, 130], [104, 98, 88]]
    } else if diamonds {
        [[255, 255, 255], [220, 250, 255], [120, 220, 255], [40, 140, 210]]
    } else {
        [[255, 255, 240], [255, 230, 120], [255, 140, 40], [170, 50, 24]]
    };
    let s = stage.min(3) as usize;
    // how much of the cell is still burning: full at first, breaking up later
    let keep = [16, 15, 11, 6][s];
    for v in 0..a {
        for u in 0..a {
            let h = hash(u + tx as i32 * a, v + ty as i32 * a, t) % 16;
            if h >= keep {
                continue;
            }
            let c = if h < 3 && s > 0 { hot[s - 1] } else { hot[s] };
            cv.block(x0 + u * k, y0 + v * k, k, c);
        }
    }
}

/// The inbox and the open exit: a doorway that flashes.
fn door(cv: &mut Canvas, x0: i32, y0: i32, a: i32, k: i32, lit: bool) {
    cv.sprite(x0, y0, k, steel(a), false, steel_pal(if lit { 0.55 } else { 0.0 }));
    if lit {
        let m = (a / 4).max(1);
        for v in m..a - m {
            for u in m..a - m {
                cv.block(x0 + u * k, y0 + v * k, k, [255, 236, 170]);
            }
        }
    }
}

/// The three steps between the inbox and the hero: a spark that gathers.
fn hatching(cv: &mut Canvas, x0: i32, y0: i32, a: i32, k: i32, n: u8) {
    let c = (a - 1) as f32 / 2.0;
    let r = c * n as f32 / 3.0 + 0.5;
    for v in 0..a {
        for u in 0..a {
            let d = (u as f32 - c).hypot(v as f32 - c);
            if d <= r {
                cv.block(x0 + u * k, y0 + v * k, k, mix([255, 176, 0], [255, 250, 220], 1.0 - d / r));
            }
        }
    }
}

// ---- drawing ---------------------------------------------------------------------

/// Draw the cave at a pixel size into `area`, the top-left of the view at cave pixel
/// `cam`. `covered` hides cells under the opening curtain; `frame` drives the purely
/// visual animation (glints, sparks), the tick count the rest.
pub fn draw(world: &World, zoom: Zoom, cam: (i32, i32), area: Rect, covered: Option<&[bool]>, frame: u64, buf: &mut Buffer) {
    let px = zoom.px();
    let (a, k) = zoom.art();
    let mut cv = Canvas { w: area.width as i32, h: area.height as i32 * 2, ox: cam.0, oy: cam.1, px: vec![SPACE; area.width as usize * area.height as usize * 2] };
    let t = world.ticks;
    let tx0 = (cam.0.max(0) / px) as usize;
    let ty0 = (cam.1.max(0) / px) as usize;
    let tx1 = (((cam.0 + cv.w) / px + 1).max(0) as usize).min(world.w);
    let ty1 = (((cam.1 + cv.h) / px + 1).max(0) as usize).min(world.h);
    for ty in ty0..ty1 {
        for tx in tx0..tx1 {
            let i = ty * world.w + tx;
            let (x0, y0) = (tx as i32 * px, ty as i32 * px);
            if covered.is_some_and(|c| c[i]) {
                cv.sprite(x0, y0, k, steel(a), false, steel_pal(0.0));
                continue;
            }
            match world.cells[i] {
                El::Space => {}
                El::Dirt => dirt(&mut cv, x0, y0, a, k, tx, ty),
                El::Steel | El::Outbox => cv.sprite(x0, y0, k, steel(a), false, steel_pal(0.0)),
                El::Brick => cv.sprite(x0, y0, k, brick(a), false, brick_pal(None)),
                El::Magic => {
                    let on = (world.magic == MagicState::Active).then_some(frame + i as u64);
                    cv.sprite(x0, y0, k, brick(a), false, brick_pal(on));
                }
                El::Boulder | El::BoulderFalling | El::Thrown(_) => cv.sprite(x0, y0, k, boulder(a), false, boulder_pal),
                El::TrollAsleep | El::TrollFalling => {
                    cv.sprite(x0, y0, k, boulder(a), false, troll_pal);
                    if crate::glyph::troll_blinks(world, i) {
                        troll_face(&mut cv, x0, y0, a, k, false, 0);
                    }
                }
                El::Troll { .. } => {
                    cv.sprite(x0, y0, k, boulder(a), false, troll_pal);
                    troll_face(&mut cv, x0, y0, a, k, true, frame / 4 + i as u64);
                }
                El::Diamond | El::DiamondFalling => {
                    let glint = (frame / 4 + i as u64 * 7) % 23 == 0;
                    cv.sprite(x0, y0, k, diamond(a), false, diamond_pal(glint));
                }
                El::Firefly(_) => firefly(&mut cv, x0, y0, a, k, frame / 2 + i as u64),
                El::Butterfly(_) => cv.sprite(x0, y0, k, butterfly(a, frame / 4 + i as u64), false, butterfly_pal),
                El::Amoeba => amoeba(&mut cv, x0, y0, a, k, tx, ty, frame),
                El::Inbox => door(&mut cv, x0, y0, a, k, t % 2 == 0),
                El::Hatching(n) => hatching(&mut cv, x0, y0, a, k, n),
                El::Player => {
                    let step = world.moved && t % 2 == 1;
                    cv.sprite(x0, y0, k, &hero(a, step), world.facing == Dir::Left, hero_pal);
                }
                El::OutboxOpen => door(&mut cv, x0, y0, a, k, t % 2 == 0),
                El::Blast { stage, diamonds, rubble } => blast(&mut cv, x0, y0, a, k, tx, ty, stage, diamonds, rubble, t),
            }
        }
    }
    // the exit opening: the whole cave flashes white for a moment
    if world.flash > 0 {
        let f = world.flash as f32 / 8.0;
        for p in &mut cv.px {
            *p = mix(*p, [255, 255, 255], f);
        }
    }
    blit(&cv, area, buf);
}

fn blit(cv: &Canvas, area: Rect, buf: &mut Buffer) {
    for cy in 0..area.height as i32 {
        for cx in 0..area.width as i32 {
            let top = cv.px[(cy * 2 * cv.w + cx) as usize];
            let bot = cv.px[((cy * 2 + 1) * cv.w + cx) as usize];
            let cell = &mut buf[(area.x + cx as u16, area.y + cy as u16)];
            cell.set_symbol("▀");
            cell.set_fg(Color::Rgb(top[0], top[1], top[2]));
            cell.set_bg(Color::Rgb(bot[0], bot[1], bot[2]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_art_set_is_square() {
        for a in [4, 6, 8] {
            let mut sets: Vec<Vec<&str>> = vec![steel(a).to_vec(), brick(a).to_vec(), boulder(a).to_vec(), diamond(a).to_vec(), hero(a, false), hero(a, true)];
            for f in 0..4 {
                sets.push(butterfly(a, f).to_vec());
            }
            for rows in sets {
                assert_eq!(rows.len() as i32, a, "{rows:?}");
                assert!(rows.iter().all(|r| r.chars().count() as i32 == a), "{rows:?}");
            }
        }
    }

    #[test]
    fn the_whole_cave_fits_when_it_can() {
        // a classic 80x24 terminal: glyphs; a big full-screen one: 6 px
        assert_eq!(Zoom::fit(40, 22, 80, 22), Zoom::GLYPH);
        assert_eq!(Zoom::fit(40, 22, 320, 73), Zoom(6));
        assert_eq!(Zoom::fit(40, 22, 160, 44), Zoom(4));
        assert_eq!(Zoom::fit_pixels(40, 22, 80, 22), Zoom(4));
        assert_eq!(Zoom::fit_pixels(40, 22, 320, 73), Zoom(6));
        assert_eq!(Zoom(4).step(-1), Zoom::GLYPH);
        assert_eq!(Zoom(24).step(1), Zoom(24));
    }
}
