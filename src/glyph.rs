//! Glyph mode: two terminal cells per tile, so a whole 40x22 cave is 80x22, exactly a
//! classic 80x25 terminal with room for the HUD. ASCII plus the shade blocks
//! (`░▒▓`, the same block range uXo's half-blocks come from).
//!
//! The hero's look is a placeholder until it is designed.

use crate::engine::{Dir, El, MagicState, World};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

const STEEL: Color = Color::Rgb(120, 124, 136);
const BRICK: Color = Color::Rgb(168, 72, 48);
const DIRT: Color = Color::Rgb(122, 86, 52);
const BOULDER: Color = Color::Rgb(190, 184, 170);
/// Troll stone: a boulder's grey with a touch of moss; amber eyes.
const TROLL: Color = Color::Rgb(160, 172, 140);
const TROLL_EYE: Color = Color::Rgb(255, 176, 40);
const TROLL_BG: Color = Color::Rgb(70, 76, 60);
const DIAMOND: Color = Color::Rgb(90, 230, 255);
const HERO: Color = Color::Rgb(255, 214, 64);
const FIREFLY: Color = Color::Rgb(255, 120, 30);
const BUTTERFLY: Color = Color::Rgb(220, 100, 255);
const AMOEBA: Color = Color::Rgb(90, 220, 90);
const AMOEBA_BG: Color = Color::Rgb(20, 60, 20);

/// Which part of the cave is on screen: the top-left tile.
#[derive(Clone, Copy, Default, Debug)]
pub struct Camera {
    pub x: usize,
    pub y: usize,
}

impl Camera {
    /// Keep the player inside a dead zone in the middle of the view.
    pub fn follow(&mut self, world: &World, view_w: usize, view_h: usize) {
        let Some(p) = world.player else { return };
        let (px, py) = (p % world.w, p / world.w);
        let axis = |cam: &mut usize, pos: usize, view: usize, size: usize| {
            if view >= size {
                *cam = 0;
                return;
            }
            let margin = view / 4;
            if pos < *cam + margin {
                *cam = pos.saturating_sub(margin);
            } else if pos + margin >= *cam + view {
                *cam = pos + margin + 1 - view;
            }
            *cam = (*cam).min(size - view);
        };
        axis(&mut self.x, px, view_w, world.w);
        axis(&mut self.y, py, view_h, world.h);
    }
}

/// Animation frame for a cell: changes every `every` ticks, offset per cell so a
/// field of the same thing doesn't blink in lockstep.
fn phase(world: &World, i: usize, every: u64) -> u64 {
    world.ticks / every + i as u64 % 3
}

/// A sleeping troll opens its eyes for a moment every few seconds, each at its own time.
pub fn troll_blinks(world: &World, i: usize) -> bool {
    (world.ticks + i as u64 * 7) % 40 < 2
}

fn look(world: &World, i: usize) -> (&'static str, Style) {
    let s = |c: Color| Style::new().fg(c);
    let t = world.ticks;
    match world.cells[i] {
        El::Space => ("  ", Style::new()),
        El::Dirt => ("░░", s(DIRT)),
        El::Steel => ("▓▓", s(STEEL)),
        El::Brick => ("▒▒", s(BRICK)),
        El::Magic => match world.magic {
            MagicState::Active => {
                let c = [Color::Rgb(255, 80, 200), Color::Rgb(120, 140, 255), Color::Rgb(80, 255, 220)][(t % 3) as usize];
                ("▒▒", s(c))
            }
            _ => ("▒▒", s(BRICK)),
        },
        El::Boulder | El::BoulderFalling | El::Thrown(_) => ("()", s(BOULDER).add_modifier(Modifier::BOLD)),
        // a sleeping troll is a boulder that now and then opens its eyes
        El::TrollAsleep | El::TrollFalling => {
            if troll_blinks(world, i) { ("oo", s(TROLL_EYE)) } else { ("()", s(TROLL).add_modifier(Modifier::BOLD)) }
        }
        El::Troll { .. } => {
            let g = ["ÒÓ", "ÓÒ"][(phase(world, i, 4) % 2) as usize];
            (g, s(TROLL_EYE).bg(TROLL_BG).add_modifier(Modifier::BOLD))
        }
        El::Diamond | El::DiamondFalling => {
            if phase(world, i, 2) % 4 == 0 { ("<>", s(Color::White).add_modifier(Modifier::BOLD)) } else { ("<>", s(DIAMOND)) }
        }
        El::Firefly(_) => {
            let g = ["[]", "][", "[]", "||"][(phase(world, i, 1) % 4) as usize];
            (g, s(FIREFLY).add_modifier(Modifier::BOLD))
        }
        El::Butterfly(_) => {
            let g = ["}{", ")(", "><", ")("][(phase(world, i, 1) % 4) as usize];
            (g, s(BUTTERFLY).add_modifier(Modifier::BOLD))
        }
        El::Amoeba => {
            let g = ["oO", "Oo", "oo", "OO"][(phase(world, i, 2) % 4) as usize];
            (g, s(AMOEBA).bg(AMOEBA_BG))
        }
        // the inbox flashes against a steel look until it opens
        El::Inbox => {
            if t % 2 == 0 { ("▓▓", s(Color::White)) } else { ("▓▓", s(STEEL)) }
        }
        El::Hatching(n) => match n {
            1 => ("..", s(HERO)),
            2 => ("oo", s(HERO)),
            _ => ("OO", s(HERO).add_modifier(Modifier::BOLD)),
        },
        El::Player => {
            let g = if world.facing == Dir::Left { "\\o" } else { "o/" };
            (g, s(HERO).add_modifier(Modifier::BOLD))
        }
        // a closed exit looks like steel
        El::Outbox => ("▓▓", s(STEEL)),
        El::OutboxOpen => {
            if t % 2 == 0 { ("▓▓", s(Color::White).add_modifier(Modifier::BOLD)) } else { ("▓▓", s(STEEL)) }
        }
        El::Blast { stage, diamonds, rubble } => {
            let c = if rubble {
                [Color::White, Color::Rgb(200, 196, 180), Color::Rgb(150, 144, 130), Color::Rgb(100, 96, 88)][stage.min(3) as usize]
            } else if diamonds {
                [Color::White, Color::White, DIAMOND, Color::Rgb(40, 140, 200)][stage.min(3) as usize]
            } else {
                [Color::White, Color::Rgb(255, 240, 120), Color::Rgb(255, 150, 40), Color::Rgb(200, 50, 30)][stage.min(3) as usize]
            };
            (["**", "**", "++", ".."][stage.min(3) as usize], s(c).add_modifier(Modifier::BOLD))
        }
    }
}

/// Draw the cave into `area` from the camera. `covered` hides cells under the
/// opening curtain (true = still covered).
pub fn draw(world: &World, cam: Camera, area: Rect, covered: Option<&[bool]>, buf: &mut Buffer) {
    let flash = world.flash > 0;
    for vy in 0..area.height as usize {
        let ty = cam.y + vy;
        if ty >= world.h {
            break;
        }
        for vx in 0..(area.width as usize / 2) {
            let tx = cam.x + vx;
            if tx >= world.w {
                break;
            }
            let i = ty * world.w + tx;
            let (g, mut st) = if covered.is_some_and(|c| c[i]) { ("▓▓", Style::new().fg(STEEL)) } else { look(world, i) };
            if flash {
                st = st.bg(Color::Rgb(230, 230, 230));
            }
            buf.set_string(area.x + (vx * 2) as u16, area.y + vy as u16, g, st);
        }
    }
}
