//! Moraen: dig through the till, collect the diamonds, get out before the cave
//! comes down on you. A spiritual successor to the 1984 cave-digging classic, with
//! its engine rebuilt faithfully and everything you see our own.
//!
//! Run: `cargo run --release [-- --cave 1-20] [--level 1-5] [--glyphs | --pixels]
//! [--zoom g|4|6|8|12|16|18|24]` (`RUST_LOG=debug cargo run --release 2>moraen.log`
//! for a session log). Graphics: `--glyphs` for characters, `--pixels` for
//! half-blocks (after uXo); without either, half-blocks when the whole cave fits and
//! characters when not. In the game `G` switches between the two, `+`/`-` change the
//! size, `0` goes back to automatic. Without a terminal: `cargo run -- --frame N
//! [--moves RRDD..] [--cave C] [--level L] [--size 80x24] [--zoom Z] [--ppm out.ppm]` plays N
//! ticks (moves `U D L R`, `.` to wait, lowercase to snap) and prints the screen as
//! text, or writes it as an image (one pixel per half-block).

mod cave;
mod engine;
mod glyph;
mod input;
mod pixel;

use cave::Cave;
use engine::{Cmd, Dir, Event, Phase, World};
use glyph::Camera;
use input::Input;
use pixel::Zoom;
use log::{debug, info};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event as TermEvent, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::io;
use std::time::{Duration, Instant};

const FRAME: Duration = Duration::from_millis(16);
const AMBER: Color = Color::Rgb(255, 176, 0);
const GREEN: Color = Color::Rgb(80, 255, 120);
const DIM: Color = Color::Rgb(120, 120, 130);
const LIVES: u32 = 3;
const MAX_LIVES: u32 = 9;
const LIFE_EVERY: u32 = 500;
/// Frames of the opening curtain before it lifts all at once (BD1: 69 rounds).
const CURTAIN_ROUNDS: u32 = 69;

fn main() -> io::Result<()> {
    env_logger::Builder::from_default_env().format_timestamp_millis().init();
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str);
    let level = arg("--level").and_then(|v| v.parse().ok()).unwrap_or(1u32).clamp(1, 5);
    // a cave by its place in the world, 1 to 20 counting intermissions
    let cave = arg("--cave").and_then(|v| v.parse::<usize>().ok()).map(|c| c.clamp(1, cave::FILES.len()) - 1);
    let flag = |name: &str| args.iter().any(|a| a == name);
    let mut gfx = if flag("--glyphs") {
        Gfx::Glyphs
    } else if flag("--pixels") {
        Gfx::Pixels
    } else {
        Gfx::Auto
    };
    let mut zoom = None;
    match arg("--zoom").and_then(Zoom::parse) {
        Some(Zoom::GLYPH) => gfx = Gfx::Glyphs,
        Some(z) => (gfx, zoom) = (Gfx::Pixels, Some(z)),
        None => {}
    }
    if let Some(n) = arg("--frame") {
        let size = arg("--size").and_then(|s| s.split_once('x')).and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
        return frame_dump(n.parse().unwrap_or(0), arg("--moves").unwrap_or(""), cave.unwrap_or(0), level, size.unwrap_or((80, 24)), (gfx, zoom), arg("--ppm"));
    }
    run(cave, level, (gfx, zoom))
}

fn seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)
}

/// Character or half-block graphics.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gfx {
    /// Half-blocks when the whole cave fits, characters when not.
    Auto,
    Glyphs,
    Pixels,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    /// The start menu: where to begin and at which level.
    Menu,
    /// The cave's name card, for a number of frames.
    Card(u32),
    Playing,
    /// Leftover seconds counting into the score.
    Bonus,
    GameOver,
}

/// Frames a cave's name card stays up (a press skips it).
const CARD_FRAMES: u32 = 110;

/// A run through the caves: lives, score, difficulty level.
struct Session {
    /// Which cave of the world, 0 to 19.
    index: usize,
    cave: Cave,
    /// The menu's choices: a start from `cave::STARTS`, and the level.
    menu_start: usize,
    menu_level: u32,
    level: u32,
    lives: u32,
    score: u32,
    next_life: u32,
    world: World,
    state: State,
    cam: Camera,
    gfx: Gfx,
    /// A half-block size picked with `+`/`-`; `None` fits the whole cave.
    zoom: Option<Zoom>,
    /// Pixel mode's view, its top-left in cave pixels, eased toward the hero.
    pcam: (f32, f32),
    snap_cam: bool,
    frames: u64,
    curtain: Vec<bool>,
    curtain_round: u32,
    curtain_rng: engine::Rng,
    paused: bool,
    acc_ms: f32,
    toast: Option<(String, u32)>,
}

impl Session {
    /// A session at the start menu.
    fn menu(level: u32) -> Session {
        let mut s = Session::new(0, level, seed());
        s.state = State::Menu;
        s.menu_level = level;
        s
    }

    fn new(index: usize, level: u32, seed: u64) -> Session {
        let cave = cave::load(index);
        let world = World::new(&cave, level, seed);
        let n = world.cells.len();
        let mut s = Session {
            index,
            cave,
            menu_start: 0,
            menu_level: level,
            level,
            lives: LIVES,
            score: 0,
            next_life: LIFE_EVERY,
            world,
            state: State::Playing,
            cam: Camera::default(),
            gfx: Gfx::Auto,
            zoom: None,
            pcam: (0.0, 0.0),
            snap_cam: true,
            frames: 0,
            curtain: vec![true; n],
            curtain_round: 0,
            curtain_rng: engine::Rng::new(seed ^ 0xC0FFEE),
            paused: false,
            acc_ms: 0.0,
            toast: None,
        };
        s.start(seed);
        s.state = State::Card(CARD_FRAMES);
        s
    }

    /// Go to cave `index` (wrapping to the next level after the last one) and show
    /// its card. Entering an intermission is worth a life.
    fn enter(&mut self, index: usize) {
        if index >= cave::FILES.len() {
            self.level = (self.level + 1).min(5);
            info!("round the world again, level {}", self.level);
        }
        self.index = index % cave::FILES.len();
        self.cave = cave::load(self.index);
        self.start(seed());
        self.state = State::Card(CARD_FRAMES);
        if self.cave.intermission && self.lives < MAX_LIVES {
            self.lives += 1;
            self.toast = Some(("BONUS LIFE".into(), 120));
            info!("bonus life for the intermission: {}", self.lives);
        }
    }

    /// (Re)start the cave at the current level.
    fn start(&mut self, seed: u64) {
        self.world = World::new(&self.cave, self.level, seed);
        self.world.score = self.score;
        self.curtain = vec![true; self.world.cells.len()];
        self.curtain_round = 0;
        self.snap_cam = true;
        self.state = State::Playing;
        self.acc_ms = 0.0;
        info!("cave {:?} level {} start: lives {}, score {}, quota {}, time {}", self.cave.name, self.level, self.lives, self.score, self.world.quota, self.world.time_left);
    }

    fn lift_curtain(&mut self) {
        if self.curtain_round > CURTAIN_ROUNDS {
            return;
        }
        self.curtain_round += 1;
        if self.curtain_round > CURTAIN_ROUNDS {
            self.curtain.fill(false);
            return;
        }
        let w = self.world.w;
        for y in 0..self.world.h {
            let x = (self.curtain_rng.next() % w as u64) as usize;
            self.curtain[y * w + x] = false;
        }
    }

    fn tick(&mut self, cmd: Cmd) {
        let before = self.world.phase;
        self.world.tick(cmd);
        for &e in &self.world.events {
            match e {
                Event::Push | Event::Dig | Event::BoulderLand | Event::DiamondLand | Event::MagicPass | Event::BoulderStart => {
                    debug!("event {e:?}")
                }
                _ => info!("event {e:?} at tick {}", self.world.ticks),
            }
        }
        if self.world.phase != before {
            info!("phase {:?} -> {:?}", before, self.world.phase);
        }
        self.score_changed();
        if self.world.phase == Phase::Won {
            self.state = State::Bonus;
            info!("cave complete with {} s left", self.world.time_left);
        }
    }

    fn score_changed(&mut self) {
        self.score = self.world.score;
        while self.score >= self.next_life {
            self.next_life += LIFE_EVERY;
            if self.lives < MAX_LIVES {
                self.lives += 1;
                self.toast = Some(("EXTRA LIFE".into(), 90));
                info!("extra life: {}", self.lives);
            }
        }
    }

    /// One frame of the bonus count: a second of leftover time into the score.
    fn bonus(&mut self) {
        if self.world.time_left > 0 {
            self.world.time_left -= 1;
            self.world.score += self.level;
            self.score_changed();
            return;
        }
        info!("cave {} done, on to the next", self.index + 1);
        self.enter(self.index + 1);
    }

    /// A press after dying or running out of time. Failing an intermission costs
    /// nothing: play goes on to the next cave.
    fn retry(&mut self) {
        if self.cave.intermission {
            info!("intermission failed, on to the next cave");
            return self.enter(self.index + 1);
        }
        self.lives = self.lives.saturating_sub(1);
        if self.lives == 0 {
            self.state = State::GameOver;
            info!("game over, score {}", self.score);
        } else {
            info!("retry, {} lives left", self.lives);
            self.start(seed());
        }
    }

    fn frame(&mut self, dt: Duration, input: &mut Input) {
        if let Some((_, n)) = &mut self.toast {
            *n = n.saturating_sub(1);
            if *n == 0 {
                self.toast = None;
            }
        }
        let pressed = std::mem::take(&mut input.pressed);
        match self.state {
            State::GameOver => {
                if pressed {
                    self.to_menu();
                    input.take_menu();
                }
                return;
            }
            State::Menu => return self.menu_input(input),
            State::Card(n) => {
                if n == 0 || pressed {
                    input.take_menu();
                    self.state = State::Playing;
                    self.acc_ms = 0.0;
                } else {
                    self.state = State::Card(n - 1);
                }
                return;
            }
            State::Bonus => return self.bonus(),
            State::Playing => {}
        }
        if self.paused {
            return;
        }
        self.lift_curtain();
        if self.world.can_retry() && pressed {
            return self.retry();
        }
        self.acc_ms += dt.as_secs_f32() * 1000.0;
        let tick = self.world.tick_ms as f32;
        // never more than a couple of ticks to catch up after a stall
        self.acc_ms = self.acc_ms.min(tick * 2.0);
        while self.acc_ms >= tick && self.state == State::Playing {
            self.acc_ms -= tick;
            let cmd = input.take_cmd(Instant::now());
            if cmd.dir.is_some() {
                debug!("tick {} cmd {:?}", self.world.ticks, cmd);
            }
            self.tick(cmd);
        }
    }

    /// Back to the start menu, keeping the graphics settings and the last choices.
    fn to_menu(&mut self) {
        let (gfx, zoom, start, level) = (self.gfx, self.zoom, self.menu_start, self.menu_level);
        *self = Session::menu(level);
        (self.gfx, self.zoom, self.menu_start) = (gfx, zoom, start);
        info!("start menu");
    }

    fn menu_input(&mut self, input: &mut Input) {
        let (dir, go) = input.take_menu();
        match dir {
            Some(Dir::Up) => self.menu_start = (self.menu_start + cave::STARTS.len() - 1) % cave::STARTS.len(),
            Some(Dir::Down) => self.menu_start = (self.menu_start + 1) % cave::STARTS.len(),
            Some(Dir::Left) => self.menu_level = (self.menu_level - 1).max(1),
            Some(Dir::Right) => self.menu_level = (self.menu_level + 1).min(5),
            None => {}
        }
        if go {
            let (gfx, zoom, start, level) = (self.gfx, self.zoom, self.menu_start, self.menu_level);
            info!("new game at {} level {}", cave::label(cave::STARTS[start]), level);
            *self = Session::new(cave::STARTS[start], level, seed());
            (self.gfx, self.zoom, self.menu_start, self.menu_level) = (gfx, zoom, start, level);
        }
    }

    fn draw_menu(&self, area: Rect, buf: &mut Buffer) {
        let amber = Style::new().fg(AMBER).add_modifier(Modifier::BOLD);
        let green = Style::new().fg(GREEN);
        let dim = Style::new().fg(DIM);
        let mid = |s: &str| area.x + area.width.saturating_sub(s.chars().count() as u16) / 2;
        let top = area.y + area.height.saturating_sub(16) / 2;
        let line = |buf: &mut Buffer, dy: u16, s: &str, st: Style| {
            if top + dy < area.y + area.height {
                buf.set_string(mid(s), top + dy, s, st);
            }
        };
        line(buf, 0, "M O R A E N", amber);
        line(buf, 2, "dig the till, gather the gems, get out", dim);
        line(buf, 4, "START AT", dim);
        for (k, &i) in cave::STARTS.iter().enumerate() {
            let c = cave::load(i);
            let sel = k == self.menu_start;
            let s = format!("{} {:<8} {:<12}{}", if sel { ">" } else { " " }, cave::label(i), c.name.to_uppercase(), if sel { "<" } else { " " });
            line(buf, 5 + k as u16, &s, if sel { amber } else { green });
        }
        line(buf, 10, &format!("LEVEL  < {} >", self.menu_level), amber);
        line(buf, 13, "up/down where   left/right level   Space or Enter to dig", dim);
        line(buf, 14, "G graphics   Esc quit", dim);
    }

    fn draw_card(&self, area: Rect, buf: &mut Buffer) {
        let amber = Style::new().fg(AMBER).add_modifier(Modifier::BOLD);
        let green = Style::new().fg(GREEN);
        let dim = Style::new().fg(DIM);
        let mid = |s: &str| area.x + area.width.saturating_sub(s.chars().count() as u16) / 2;
        let y = area.y + area.height.saturating_sub(7) / 2;
        let rows: [(String, Style); 5] = [
            (cave::label(self.index), dim),
            (self.cave.name.to_uppercase(), amber),
            (self.cave.about.clone(), green),
            (format!("level {}   {} diamonds   {} seconds   {} men", self.level, self.world.quota, self.world.time_left, self.lives), dim),
            (self.toast.as_ref().map_or(String::new(), |t| t.0.clone()), amber),
        ];
        for (k, (s, st)) in rows.iter().enumerate() {
            let dy = [0, 2, 4, 6, 8][k];
            if y + dy < area.y + area.height {
                buf.set_string(mid(s), y + dy, s, *st);
            }
        }
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, Style::new().bg(Color::Black));
        if area.height < 3 || area.width < 20 {
            return;
        }
        match self.state {
            State::Menu => return self.draw_menu(area, buf),
            State::Card(_) => return self.draw_card(area, buf),
            _ => {}
        }
        self.frames += 1;
        let (room_w, room_h) = (area.width as usize, area.height as usize - 2);
        let zoom = self.zoom_for(room_w, room_h);
        let covered = (self.curtain_round <= CURTAIN_ROUNDS).then_some(self.curtain.as_slice());
        let cave_area = if zoom == Zoom::GLYPH {
            let w = &self.world;
            let view_w = (room_w / 2).min(w.w);
            let view_h = room_h.min(w.h);
            self.cam.follow(w, view_w, view_h);
            let r = centred(area, (view_w * 2) as u16, view_h as u16);
            glyph::draw(w, self.cam, r, covered, buf);
            r
        } else {
            let px = zoom.px();
            let (cave_w, cave_h) = (self.world.w as i32 * px, self.world.h as i32 * px);
            let view_w = (room_w as i32).min(cave_w);
            let view_h = (room_h as i32).min((cave_h + 1) / 2);
            let r = centred(area, view_w as u16, view_h as u16);
            let cam = self.pixel_camera(px, (cave_w, cave_h), (view_w, view_h * 2));
            let covered = (self.curtain_round <= CURTAIN_ROUNDS).then_some(self.curtain.as_slice());
            pixel::draw(&self.world, zoom, cam, r, covered, self.frames, buf);
            r
        };
        let w = &self.world;
        let (x0, y0, view_h) = (cave_area.x, cave_area.y, cave_area.height as usize);

        // the HUD, above the cave
        let hud_y = y0 - 1;
        let amber = Style::new().fg(AMBER).add_modifier(Modifier::BOLD);
        let green = Style::new().fg(GREEN);
        let dim = Style::new().fg(DIM);
        let mut x = x0;
        let mut put = |s: &str, st: Style| {
            buf.set_string(x, hud_y, s, st);
            x += s.chars().count() as u16;
        };
        let got = format!("<> {:02}/{:02}", w.diamonds, w.quota);
        put(&got, if w.exit_open() { amber } else { Style::new().fg(Color::Rgb(90, 230, 255)) });
        put(&format!(" x{:<3}", w.value), dim);
        let time = match w.phase {
            Phase::Hatching => "READY".to_string(),
            _ => format!("{:03}", w.time_left),
        };
        put(" TIME ", dim);
        put(&time, if w.phase == Phase::Playing && w.time_left <= 10 { amber } else { green });
        put(" SCORE ", dim);
        put(&format!("{:06}", self.score), green);
        put(" MEN ", dim);
        put(&self.lives.to_string(), green);
        put(&format!(" L{}", self.level), dim);
        let name = format!(" {} {}", cave::label(self.index), self.cave.name.to_uppercase());
        let right = (x0 + cave_area.width).saturating_sub(name.chars().count() as u16);
        if right > x {
            buf.set_string(right, hud_y, &name, amber);
        }

        // the message line, below
        let msg_y = y0 + view_h as u16;
        let msg: Option<(String, Style)> = match (self.state, w.phase) {
            (State::GameOver, _) => Some((format!("GAME OVER  score {}   press any key", self.score), amber)),
            (State::Bonus, _) => Some(("CAVE CLEARED".into(), amber)),
            _ if self.paused => Some(("PAUSED  P to go on".into(), amber)),
            (_, Phase::TimeUp) => Some(("OUT OF TIME   press any key".into(), amber)),
            (_, Phase::Dead) if w.can_retry() => Some(("press any key".into(), amber)),
            _ => self.toast.as_ref().map(|(t, _)| (t.clone(), amber)),
        };
        let line = msg.unwrap_or_else(|| ("arrows/WASD dig   Shift+dir or Space snap   G graphics   +/- zoom   P pause   Esc quit".into(), dim));
        let lx = x0 + cave_area.width.saturating_sub(line.0.chars().count() as u16) / 2;
        if msg_y < area.y + area.height {
            buf.set_string(lx.max(area.x), msg_y, &line.0, line.1);
        }
    }

    /// The size to draw at in a view of `w` x `h` cells.
    fn zoom_for(&self, w: usize, h: usize) -> Zoom {
        let (cw, ch) = (self.world.w, self.world.h);
        match self.gfx {
            Gfx::Glyphs => Zoom::GLYPH,
            Gfx::Pixels => self.zoom.unwrap_or_else(|| Zoom::fit_pixels(cw, ch, w, h)),
            Gfx::Auto => self.zoom.unwrap_or_else(|| Zoom::fit(cw, ch, w, h)),
        }
    }

    /// `G`: characters to half-blocks (at the fitting size) and back.
    fn toggle_gfx(&mut self, w: usize, h: usize) {
        self.gfx = if self.zoom_for(w, h) == Zoom::GLYPH { Gfx::Pixels } else { Gfx::Glyphs };
        self.zoom = None;
        self.snap_cam = true;
        info!("graphics {:?}", self.gfx);
    }

    /// `+`/`-`: step the size; stepping below 4 px is characters.
    fn step_zoom(&mut self, by: i32, w: usize, h: usize) {
        match self.zoom_for(w, h).step(by) {
            Zoom::GLYPH => (self.gfx, self.zoom) = (Gfx::Glyphs, None),
            z => (self.gfx, self.zoom) = (Gfx::Pixels, Some(z)),
        }
        self.snap_cam = true;
        info!("graphics {:?} zoom {}", self.gfx, self.zoom.map_or("fit".into(), |z| z.name()));
    }

    /// Keep the hero inside a dead zone in the middle of the view, easing there; a
    /// cave smaller than the view sits still.
    fn pixel_camera(&mut self, px: i32, (cw, ch): (i32, i32), (vw, vh): (i32, i32)) -> (i32, i32) {
        let (mut tx, mut ty) = self.pcam;
        if let Some(p) = self.world.player {
            let (hx, hy) = ((p % self.world.w) as i32 * px + px / 2, (p / self.world.w) as i32 * px + px / 2);
            let axis = |cam: &mut f32, pos: i32, view: i32| {
                let margin = view / 4;
                if (pos as f32) < *cam + margin as f32 {
                    *cam = (pos - margin) as f32;
                } else if (pos as f32) > *cam + (view - margin) as f32 {
                    *cam = (pos - view + margin) as f32;
                }
            };
            if self.snap_cam {
                // a new cave starts centred on the hero (or the inbox)
                (tx, ty) = ((hx - vw / 2) as f32, (hy - vh / 2) as f32);
            } else {
                axis(&mut tx, hx, vw);
                axis(&mut ty, hy, vh);
            }
        } else if self.snap_cam {
            let inbox = self.world.cells.iter().position(|&e| e == engine::El::Inbox).unwrap_or(0);
            let (ix, iy) = ((inbox % self.world.w) as i32 * px, (inbox / self.world.w) as i32 * px);
            (tx, ty) = ((ix - vw / 2) as f32, (iy - vh / 2) as f32);
        }
        tx = tx.clamp(0.0, (cw - vw).max(0) as f32);
        ty = ty.clamp(0.0, (ch - vh).max(0) as f32);
        if self.snap_cam {
            self.pcam = (tx, ty);
            self.snap_cam = self.world.player.is_none() && self.world.phase == Phase::Hatching;
        } else {
            self.pcam.0 += (tx - self.pcam.0) * 0.2;
            self.pcam.1 += (ty - self.pcam.1) * 0.2;
        }
        (self.pcam.0.round() as i32, self.pcam.1.round() as i32)
    }

    fn snapshot_log(&self) {
        let w = &self.world;
        debug!(
            "state {:?} phase {:?} tick {} player {:?} facing {:?} diamonds {}/{} time {} score {} lives {} magic {:?} amoeba_fast {} paused {}",
            self.state,
            w.phase,
            w.ticks,
            w.player.map(|p| (p % w.w, p / w.w)),
            w.facing,
            w.diamonds,
            w.quota,
            w.time_left,
            self.score,
            self.lives,
            w.magic,
            w.amoeba_fast,
            self.paused
        );
    }
}

/// A `w` x `h` rectangle centred in `area`, one row in from the top for the HUD.
fn centred(area: Rect, w: u16, h: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + 1 + (area.height - 2).saturating_sub(h) / 2;
    Rect::new(x, y, w.min(area.width), h.min(area.height - 2))
}

fn run(cave: Option<usize>, level: u32, (gfx, zoom): (Gfx, Option<Zoom>)) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let kitty = supports_keyboard_enhancement().unwrap_or(false);
    if kitty {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES | KeyboardEnhancementFlags::REPORT_EVENT_TYPES;
        execute!(io::stdout(), PushKeyboardEnhancementFlags(flags))?;
    }
    info!("start: kitty keyboard = {kitty}, level {level}");
    let mut input = Input::new(kitty);
    let mut session = match cave {
        Some(i) => Session::new(i, level, seed()),
        None => Session::menu(level),
    };
    (session.gfx, session.zoom) = (gfx, zoom);
    let mut last = Instant::now();
    let mut last_log = Instant::now();

    let result = loop {
        let deadline = last + FRAME;
        while event::poll(deadline.saturating_duration_since(Instant::now()))? {
            if let TermEvent::Key(k) = event::read()? {
                input.on_key(k, Instant::now());
            }
        }
        input.poll_pads();
        if input.quit {
            break Ok(());
        }
        // Esc: out of a game to the menu, and from the menu out of the game
        if std::mem::take(&mut input.back) {
            if session.state == State::Menu {
                break Ok(());
            }
            session.to_menu();
        }
        if std::mem::take(&mut input.pause) && session.state == State::Playing {
            session.paused = !session.paused;
            input.pressed = false;
            info!("{}", if session.paused { "paused" } else { "resumed" });
        }
        let size = terminal.size()?;
        let room = (size.width as usize, (size.height as usize).saturating_sub(2));
        if std::mem::take(&mut input.zoom_auto) {
            (session.gfx, session.zoom) = (Gfx::Auto, None);
            session.snap_cam = true;
            info!("graphics auto");
        }
        if std::mem::take(&mut input.toggle_gfx) {
            session.toggle_gfx(room.0, room.1);
        }
        if input.zoom != 0 {
            session.step_zoom(std::mem::take(&mut input.zoom), room.0, room.1);
        }
        let now = Instant::now();
        let dt = now.duration_since(last);
        last = now;
        session.frame(dt, &mut input);
        if now.duration_since(last_log) >= Duration::from_millis(500) {
            last_log = now;
            session.snapshot_log();
        }
        if let Err(e) = terminal.draw(|f| session.draw(f.area(), f.buffer_mut())) {
            break Err(e);
        }
    };
    if kitty {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    info!("quit, score {}", session.score);
    result
}

/// Play `n` ticks headless and print the screen as plain text.
fn frame_dump(n: u64, moves: &str, cave: usize, level: u32, (w, h): (u16, u16), (gfx, zoom): (Gfx, Option<Zoom>), ppm: Option<&str>) -> io::Result<()> {
    let mut s = Session::new(cave, level, 1);
    s.state = State::Playing;
    (s.gfx, s.zoom) = (gfx, zoom);
    s.curtain_round = CURTAIN_ROUNDS + 1;
    let mut moves = moves.chars();
    for _ in 0..n {
        let cmd = match moves.next() {
            Some(c) => {
                let dir = match c.to_ascii_uppercase() {
                    'U' => Some(Dir::Up),
                    'D' => Some(Dir::Down),
                    'L' => Some(Dir::Left),
                    'R' => Some(Dir::Right),
                    _ => None,
                };
                Cmd { dir, snap: c.is_ascii_lowercase() }
            }
            None => Cmd::default(),
        };
        if s.state == State::Playing {
            s.tick(cmd);
        }
    }
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    s.draw(area, &mut buf);
    if let Some(path) = ppm {
        // one pixel per half-block: a cell is the top and bottom pixel
        let rgb = |c: Color| match c {
            Color::Rgb(r, g, b) => [r, g, b],
            Color::Reset | Color::Black => [0, 0, 0],
            _ => [200, 200, 200],
        };
        let mut out = format!("P6\n{w} {}\n255\n", h * 2).into_bytes();
        for y in 0..h * 2 {
            for x in 0..w {
                let cell = &buf[(x, y / 2)];
                let c = if cell.symbol() == "▀" {
                    if y % 2 == 0 { rgb(cell.fg) } else { rgb(cell.bg) }
                } else if cell.symbol().trim().is_empty() {
                    rgb(cell.bg)
                } else {
                    rgb(cell.fg)
                };
                out.extend_from_slice(&c);
            }
        }
        return std::fs::write(path, out);
    }
    for y in 0..h {
        let line: String = (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect();
        println!("{}", line.trim_end());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caves_follow_in_order_and_intermissions_give_a_life() {
        let mut s = Session::new(3, 1, 1);
        assert_eq!(s.lives, LIVES);
        s.enter(4);
        assert!(s.cave.intermission);
        assert_eq!(s.lives, LIVES + 1, "entering an intermission is worth a life");
        assert!(matches!(s.state, State::Card(_)));
    }

    #[test]
    fn failing_an_intermission_costs_nothing() {
        let mut s = Session::new(4, 1, 1);
        let lives = s.lives;
        s.retry();
        assert_eq!(s.index, 5, "on to the next cave");
        assert_eq!(s.lives, lives);
        // a normal cave costs a life and is played again
        s.retry();
        assert_eq!((s.index, s.lives), (5, lives - 1));
    }

    #[test]
    fn after_the_last_cave_the_world_starts_again_one_level_up() {
        let mut s = Session::new(19, 2, 1);
        s.enter(20);
        assert_eq!((s.index, s.level), (0, 3));
        let mut s = Session::new(19, 5, 1);
        s.enter(20);
        assert_eq!(s.level, 5, "five stays five");
    }

    #[test]
    fn the_last_life_lost_is_game_over() {
        let mut s = Session::new(0, 1, 1);
        s.lives = 1;
        s.retry();
        assert_eq!(s.state, State::GameOver);
    }
}
