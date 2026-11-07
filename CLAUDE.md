# Blastbeat

Blast-beat MIDI generator (CLAP/VST3 note effect) in Rust using nih-plug, no audio I/O.
The user runs it in Bitwig Studio on Linux (CLAP), feeding the Drum Machine. Scaffolding and CI are copied from `../vibe`.

## Commands

```sh
cargo xtask bundle blastbeat --release && cp -r target/bundled/Blastbeat.clap ~/.clap/   # build + install
cargo test --release --lib
cargo clippy --release --lib
```

- The system has an old distro Rust (1.75) in `/usr/bin` that comes before `~/.cargo/bin` on PATH and
  can't build nih-plug. Prefix commands with `PATH="$HOME/.cargo/bin:$PATH"`.
- Bitwig needs the plugin re-added or a restart to pick up a new build.
- Add user-facing changes to `CHANGELOG.md` under `[Unreleased]` as you make them.
- Releases: follow `docs/RELEASING.md` (CI publishes on annotated `v*` tags; never move an existing tag).
- The "trailing semicolon in macro" warnings come from nih-plug's export macros, not our code.

## Structure

- `pattern.rs`: `step_hits(bar, step, pattern)` gives each voice's velocity. All randomness goes through
  `random(seed, bar, step, voice, stream)`; give new features a new stream so existing patterns don't reshuffle.
  `section_at(bar, pattern)` draws each block's section independently; a Markov chain would need replaying from bar 0.
- `feel.rs`: `offset_ms` and `velocity`, also pure. Drift is value noise rather than a stateful walk.
  `MAX_OFFSET_MS` must cover every offset, because the scheduler's lookahead depends on it.
- `mistakes.rs`: the amateur layer, also pure. `MAX_EARLY_MS` adds to the lookahead the same way `MAX_OFFSET_MS` does.
- `midi.rs`: `Scheduler` finds hits due in the block and emits sorted events. Its only state is sounding notes and
  where the last block ended; it must never affect which hits exist.
- `params.rs`: params and enums. `editor.rs`: generic param list plus Reroll. A plugin can only set its own params
  from the GUI, so Reroll has to live there.
- The audio thread must not allocate (`assert_process_allocs` is enabled).
- `CLAP_ID`, `VST3_CLASS_ID` and param IDs must never change, or saved Bitwig projects lose the plugin or its settings.

## Planned

Keep these possible: double bass, riff-follow, tempo ramps. They fit as new voices
(double bass) and a `MIDI_INPUT` (riff-follow).
