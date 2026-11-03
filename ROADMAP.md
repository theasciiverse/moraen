# Roadmap

Moraen is at 0.4.0 (2026-10-28): the engine is complete and the first world
is playable; the rest of the approved plan (2026-10-05) is still to build.
1.0.0 is when everything below is in.

## 0.4.0: built

- [x] The BD1 engine: every BD1 element and its rules, under test
- [x] Glyph and half-block graphics, switchable in the game
- [x] Keyboard and gamepad; session logging
- [x] The first world: 20 caves and 5 intermissions, the stone trolls in the
  last four, five difficulty levels, a start menu and cave cards

## 0.4.1: built

- [x] Gamepads work on Windows: gilrs reads them through XInput, since its
  default (Windows Gaming Input) only reports to a program with a focused
  window of its own, which a terminal game never has

## 0.5.0: look and feel

- [ ] Sound and music, our own
- [ ] A proper title screen and results screens
- [ ] A pixel-font HUD
- [ ] The hero's idle animation

## 0.6.0: other people's caves

- [ ] Import: BDCFF, GDash `.bd` and Rocks'n'Diamonds level sets
- [ ] The cave editor, saving our own text format

## 0.7.0: more worlds

- [ ] Slime and expanding wall (BD2), and a world that brings them in
- [ ] Picks from the later games (keys and doors, conveyors, gravity switch,
  bombs, acid, ...), a few per world
- [ ] The terminal worlds: a cave that scrolls up like output, glitch tiles,
  a cursor that types new boulders in

## 0.8.0: two players

- [ ] Co-op: two diggers in one cave, a shared quota
- [ ] Race over iroh: same cave and seed side by side, first out wins

## 1.0.0

- [ ] Everything above played through and tuned
- [ ] The engine conflicts confirmed (amoeba timer start, hatching delay;
  see the open questions in `CLAUDE.md`)
