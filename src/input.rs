//! Controls, after uXo's: keyboard (WASD or arrows, Space or Enter to snap) and every
//! gamepad (stick or d-pad to move, any face button to snap). One digger for now, so
//! every control drives it; co-op seats come later.
//!
//! The cave reads input once per tick, as the original did, but a press is latched
//! until the next tick reads it, so a tap shorter than a tick still moves one cell.
//!
//! Snap is Shift with a direction (Shift+arrows, or capital WASD), which works in any
//! terminal. Space or Enter held works too with the kitty keyboard protocol; without
//! it a terminal only repeats the last key held, so Space instead arms one snap for
//! the next direction.
//!
//! gilrs counts any device with buttons and two axes as a gamepad, so only devices
//! that matched SDL's controller database are used; `MORAEN_ANY_PAD=1` takes all.

use crate::engine::{Cmd, Dir};
use log::{debug, info};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

/// Without release events: a key's events closer together than this are OS
/// repeats of one held key (they come every 30 to 50 ms); anything slower is a
/// fresh tap, latched for exactly one step ...
const REPEAT_GAP: Duration = Duration::from_millis(100);
/// ... and while its repeats keep coming it counts as held this long after the last
/// one. So a tap never walks two cells; holding stutters for the OS's first repeat
/// delay instead (the kitty protocol has real releases and no stutter).
const HOLD_REPEAT: Duration = Duration::from_millis(120);
/// How long an armed snap (Space without the kitty protocol) waits for a direction.
const SNAP_ARMED: Duration = Duration::from_millis(600);
const STICK_DEADZONE: f32 = 0.4;

const DIRS: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];

#[derive(Clone, Copy, Default, Debug)]
struct Key {
    down: bool,
    last: Option<Instant>,
    repeats: u32,
    /// Pressed since the cave last read it.
    latched: bool,
    shift: bool,
}

impl Key {
    fn held(&self, exact: bool, now: Instant) -> bool {
        if exact {
            return self.down;
        }
        let Some(last) = self.last else { return false };
        self.repeats > 0 && now.duration_since(last) <= HOLD_REPEAT
    }
}

pub struct Input {
    exact: bool,
    /// Up, down, left, right.
    keys: [Key; 4],
    snap_down: bool,
    snap_armed: Option<Instant>,
    pad_dirs: [bool; 4],
    pad_latched: [bool; 4],
    pad_snap: bool,
    pad_snap_latched: bool,
    stick: (f32, f32),
    /// Any press at all this frame (to end a cave after dying, skip a card).
    pub pressed: bool,
    pub quit: bool,
    pub back: bool,
    pub pause: bool,
    /// Zoom steps asked for (`+` in, `-` out), and `0` for automatic.
    pub zoom: i32,
    pub zoom_auto: bool,
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
}

fn key_dir(code: KeyCode) -> Option<usize> {
    match code {
        KeyCode::Char('w' | 'W') | KeyCode::Up => Some(0),
        KeyCode::Char('s' | 'S') | KeyCode::Down => Some(1),
        KeyCode::Char('a' | 'A') | KeyCode::Left => Some(2),
        KeyCode::Char('d' | 'D') | KeyCode::Right => Some(3),
        _ => None,
    }
}

#[cfg(feature = "gamepad")]
fn usable_pad(p: &gilrs::Gamepad) -> bool {
    p.mapping_source() == gilrs::MappingSource::SdlMappings || std::env::var("MORAEN_ANY_PAD").is_ok_and(|v| v == "1")
}

impl Input {
    pub fn new(exact: bool) -> Input {
        #[cfg(feature = "gamepad")]
        let gilrs = match gilrs::Gilrs::new() {
            Ok(g) => {
                let all: Vec<_> = g.gamepads().map(|(_, p)| (p.name().to_string(), usable_pad(&p))).collect();
                info!("gamepads at start (name, used): {all:?}");
                Some(g)
            }
            Err(e) => {
                log::warn!("gamepad support unavailable: {e}");
                None
            }
        };
        Input {
            exact,
            keys: Default::default(),
            snap_down: false,
            snap_armed: None,
            pad_dirs: [false; 4],
            pad_latched: [false; 4],
            pad_snap: false,
            pad_snap_latched: false,
            stick: (0.0, 0.0),
            pressed: false,
            quit: false,
            back: false,
            pause: false,
            zoom: 0,
            zoom_auto: false,
            #[cfg(feature = "gamepad")]
            gilrs,
        }
    }

    pub fn on_key(&mut self, k: KeyEvent, now: Instant) {
        debug!("key {:?} {:?} {:?}", k.code, k.kind, k.modifiers);
        let down = k.kind != KeyEventKind::Release;
        let press = k.kind == KeyEventKind::Press;
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if let Some(d) = key_dir(k.code) {
            let shift = k.modifiers.contains(KeyModifiers::SHIFT) || matches!(k.code, KeyCode::Char(c) if c.is_ascii_uppercase());
            let key = &mut self.keys[d];
            if self.exact {
                if press {
                    key.latched = true;
                }
                key.down = down;
                key.shift = shift;
            } else {
                let fresh = key.last.is_none_or(|l| now.duration_since(l) > REPEAT_GAP);
                key.repeats = if fresh { 0 } else { key.repeats + 1 };
                key.last = Some(now);
                key.shift = shift;
                if fresh {
                    key.latched = true;
                }
                // a press the other way ends this one at once
                self.keys[d ^ 1] = Key::default();
            }
            if press {
                self.pressed = true;
            }
            return;
        }
        match k.code {
            KeyCode::Char(' ') | KeyCode::Enter => {
                if self.exact {
                    self.snap_down = down;
                }
                if press {
                    self.snap_armed = Some(now);
                    self.pressed = true;
                }
            }
            KeyCode::Esc if press => self.back = true,
            KeyCode::Char('p' | 'P') if press => self.pause = true,
            KeyCode::Char('+' | '=') if press => self.zoom += 1,
            KeyCode::Char('-') if press => self.zoom -= 1,
            KeyCode::Char('0') if press => self.zoom_auto = true,
            _ => {}
        }
    }

    /// Drain gamepad events (call once per frame).
    pub fn poll_pads(&mut self) {
        #[cfg(feature = "gamepad")]
        {
            use gilrs::{Axis, Button, EventType};
            let Some(g) = &mut self.gilrs else { return };
            while let Some(ev) = g.next_event() {
                let pad = g.gamepad(ev.id);
                if !usable_pad(&pad) {
                    continue;
                }
                match ev.event {
                    EventType::Connected => info!("pad connected: {}", pad.name()),
                    EventType::Disconnected => {
                        info!("pad disconnected: {}", pad.name());
                        self.pad_dirs = [false; 4];
                        self.stick = (0.0, 0.0);
                        self.pad_snap = false;
                    }
                    EventType::ButtonPressed(b, _) => {
                        debug!("pad press {b:?}");
                        self.pressed = true;
                        match b {
                            Button::DPadUp | Button::DPadDown | Button::DPadLeft | Button::DPadRight => {
                                let d = match b {
                                    Button::DPadUp => 0,
                                    Button::DPadDown => 1,
                                    Button::DPadLeft => 2,
                                    _ => 3,
                                };
                                self.pad_dirs[d] = true;
                                self.pad_latched[d] = true;
                            }
                            Button::South | Button::East | Button::West | Button::North => {
                                self.pad_snap = true;
                                self.pad_snap_latched = true;
                            }
                            Button::Start => self.pause = true,
                            Button::Select => self.back = true,
                            _ => {}
                        }
                    }
                    EventType::ButtonReleased(b, _) => {
                        debug!("pad release {b:?}");
                        match b {
                            Button::DPadUp => self.pad_dirs[0] = false,
                            Button::DPadDown => self.pad_dirs[1] = false,
                            Button::DPadLeft => self.pad_dirs[2] = false,
                            Button::DPadRight => self.pad_dirs[3] = false,
                            Button::South | Button::East | Button::West | Button::North => self.pad_snap = false,
                            _ => {}
                        }
                    }
                    EventType::AxisChanged(Axis::LeftStickX | Axis::LeftStickY, ..) => {
                        let axis = |a: Axis| pad.axis_data(a).map_or(0.0, |d| d.value());
                        // stick up is positive; screen up is negative
                        let s = (axis(Axis::LeftStickX), -axis(Axis::LeftStickY));
                        let was = self.stick.0.hypot(self.stick.1) >= STICK_DEADZONE;
                        self.stick = s;
                        let is = s.0.hypot(s.1) >= STICK_DEADZONE;
                        if is != was {
                            debug!("pad stick {:.2},{:.2}", s.0, s.1);
                        }
                        if is && !was {
                            self.pressed = true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Read the controls for one tick of the cave, and clear the latches.
    pub fn take_cmd(&mut self, now: Instant) -> Cmd {
        let mut held = [false; 4];
        let mut shift = false;
        for d in 0..4 {
            let k = &mut self.keys[d];
            if k.held(self.exact, now) || k.latched {
                held[d] = true;
                shift |= k.shift;
            }
            k.latched = false;
            if self.pad_dirs[d] || self.pad_latched[d] {
                held[d] = true;
            }
            self.pad_latched[d] = false;
        }
        let (x, y) = self.stick;
        if x.hypot(y) >= STICK_DEADZONE {
            // the stick's diagonals hand the move to whichever axis leans further
            if x.abs() >= y.abs() {
                held[if x < 0.0 { 2 } else { 3 }] = true;
            } else {
                held[if y < 0.0 { 0 } else { 1 }] = true;
            }
        }
        // the 1984 joystick rule: on a diagonal the horizontal wins
        let dir = if held[2] != held[3] {
            Some(if held[2] { DIRS[2] } else { DIRS[3] })
        } else if held[0] != held[1] {
            Some(if held[0] { DIRS[0] } else { DIRS[1] })
        } else {
            None
        };
        let armed = self.snap_armed.is_some_and(|t| now.duration_since(t) <= SNAP_ARMED);
        let snap = shift || self.snap_down || self.pad_snap || self.pad_snap_latched || (!self.exact && armed);
        self.pad_snap_latched = false;
        if dir.is_some() && !self.exact {
            self.snap_armed = None;
        }
        Cmd { dir, snap }
    }
}
