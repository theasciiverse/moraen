//! Moraen: dig through the till, collect the diamonds, get out before the cave
//! comes down on you. A spiritual successor to the 1984 cave-digging classic, with
//! its engine rebuilt faithfully and everything you see our own.
//!
//! Run: `cargo run --release [-- --level 1-5]` (`RUST_LOG=debug cargo run --release
//! 2>moraen.log` for a session log). Without a terminal: `cargo run -- --frame N
//! [--moves RRDD..] [--level L] [--size 80x24]` plays N ticks (moves `U D L R`, `.`
//! to wait, lowercase to snap) and prints the screen as text.

mod cave;
mod engine;
mod glyph;
mod input;

use cave::Cave;
use engine::{Cmd, Dir, Event, Phase, World};
use glyph::Camera;
use input::Input;
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
    if let Some(n) = arg("--frame") {
        let size = arg("--size").and_then(|s| s.split_once('x')).and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
        return frame_dump(n.parse().unwrap_or(0), arg("--moves").unwrap_or(""), level, size.unwrap_or((80, 24)));
    }
    run(level)
}

fn seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Playing,
    /// Leftover seconds counting into the score.
    Bonus,
    GameOver,
}

/// A run through the caves: lives, score, difficulty level.
struct Session {
    cave: Cave,
    level: u32,
    lives: u32,
    score: u32,
    next_life: u32,
    world: World,
    state: State,
    cam: Camera,
    curtain: Vec<bool>,
    curtain_round: u32,
    curtain_rng: engine::Rng,
    paused: bool,
    acc_ms: f32,
    toast: Option<(String, u32)>,
}

impl Session {
    fn new(level: u32, seed: u64) -> Session {
        let cave = cave::load(0);
        let world = World::new(&cave, level, seed);
        let n = world.cells.len();
        let mut s = Session {
            cave,
            level,
            lives: LIVES,
            score: 0,
            next_life: LIFE_EVERY,
            world,
            state: State::Playing,
            cam: Camera::default(),
            curtain: vec![true; n],
            curtain_round: 0,
            curtain_rng: engine::Rng::new(seed ^ 0xC0FFEE),
            paused: false,
            acc_ms: 0.0,
            toast: None,
        };
        s.start(seed);
        s
    }

    /// (Re)start the cave at the current level.
    fn start(&mut self, seed: u64) {
        self.world = World::new(&self.cave, self.level, seed);
        self.world.score = self.score;
        self.curtain = vec![true; self.world.cells.len()];
        self.curtain_round = 0;
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
        self.level = (self.level + 1).min(5);
        info!("on to level {}", self.level);
        self.start(seed());
    }

    /// A press after dying or running out of time.
    fn retry(&mut self) {
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
                    *self = Session::new(1, seed());
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

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, Style::new().bg(Color::Black));
        if area.height < 3 || area.width < 20 {
            return;
        }
        let w = &self.world;
        let view_w = (area.width as usize / 2).min(w.w);
        let view_h = (area.height as usize - 2).min(w.h);
        self.cam.follow(w, view_w, view_h);
        let x0 = area.x + (area.width - (view_w * 2) as u16) / 2;
        let y0 = area.y + 1 + ((area.height - 2) - view_h as u16) / 2;
        let cave_area = Rect::new(x0, y0, (view_w * 2) as u16, view_h as u16);
        let covered = (self.curtain_round <= CURTAIN_ROUNDS).then_some(self.curtain.as_slice());
        glyph::draw(w, self.cam, cave_area, covered, buf);

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
        let name = format!(" {}", self.cave.name.to_uppercase());
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
        let line = msg.unwrap_or_else(|| ("arrows/WASD dig   Shift+dir or Space snap   P pause   Esc quit".into(), dim));
        let lx = x0 + cave_area.width.saturating_sub(line.0.chars().count() as u16) / 2;
        if msg_y < area.y + area.height {
            buf.set_string(lx.max(area.x), msg_y, &line.0, line.1);
        }
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

fn run(level: u32) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let kitty = supports_keyboard_enhancement().unwrap_or(false);
    if kitty {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES | KeyboardEnhancementFlags::REPORT_EVENT_TYPES;
        execute!(io::stdout(), PushKeyboardEnhancementFlags(flags))?;
    }
    info!("start: kitty keyboard = {kitty}, level {level}");
    let mut input = Input::new(kitty);
    let mut session = Session::new(level, seed());
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
        if input.quit || std::mem::take(&mut input.back) {
            break Ok(());
        }
        if std::mem::take(&mut input.pause) && session.state == State::Playing {
            session.paused = !session.paused;
            input.pressed = false;
            info!("{}", if session.paused { "paused" } else { "resumed" });
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
fn frame_dump(n: u64, moves: &str, level: u32, (w, h): (u16, u16)) -> io::Result<()> {
    let mut s = Session::new(level, 1);
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
    for y in 0..h {
        let line: String = (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect();
        println!("{}", line.trim_end());
    }
    Ok(())
}
