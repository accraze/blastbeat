# Blastbeat

[![CI](https://github.com/accraze/blastbeat/actions/workflows/ci.yml/badge.svg)](https://github.com/accraze/blastbeat/actions/workflows/ci.yml)

A MIDI note effect (CLAP / VST3) that generates blast-beat drum patterns from the host transport,
built with [nih-plug](https://github.com/robbert-vdh/nih-plug). Put it before Bitwig's Drum Machine
and press play. It has no audio I/O and ignores incoming notes.

The pattern is a pure function of bar, step, seed and params, with no running random state, so
looping, scrubbing and starting mid-song all produce the same hits.

## Parameters

| Param       | ID          | Range                       | Default     | What it does |
|-------------|-------------|-----------------------------|-------------|--------------|
| Intensity   | `intensity` | 0–100%                      | 100%        | Sparse to full blast. Raising it only ever adds hits. At 0% only the first step of the bar (and any crash) remains |
| Blast       | `blast`     | Traditional, Hammer         | Traditional | Traditional alternates kick and snare; Hammer hits them together |
| Division    | `division`  | 1/8, 1/16, 1/16T, 1/32      | 1/16        | Step size. The grid restarts on each bar line |
| Cymbal      | `cymbal`    | Hat, Ride, China            | Ride        | Plays on every step, accented on beat 1 |
| Crash Every | `crash`     | Off, 1–16 bars              | 4           | Crash on the first step of every Nth bar |
| Section Bars | `sectionbars` | 1–32                     | 4           | Length of each section block |
| Half-Time   | `halftime`  | 0–100%                      | 0%          | Chance a block is half-time |
| Breakdown   | `breakdown` | 0–100%                      | 0%          | Chance a block is a breakdown |
| Fill        | `fill`      | 0–100%                      | 0%          | Chance a block's last bar is a fill |
| Human       | `human`     | 0–100%                      | 50%         | Machine-to-human macro: scales Jitter and Drift. 0% is dead on the grid |
| Jitter      | `jitter`    | 0–100%                      | 50%         | Independent per-limb randomness, up to ±15 ms and ±0.2 velocity |
| Drift       | `drift`     | 0–100%                      | 30%         | Timing wobble shared by all limbs, up to ±30 ms: the tape-warped drum machine |
| Drift Rate  | `driftrate` | 0.1–8 per bar               | 0.5         | How fast Drift wanders |
| Amateur     | `amateur`   | 0–100%                      | 0%          | Macro: scales the five mistakes below. 0% turns them all off |
| Rush        | `rush`      | 0–100%                      | 50%         | Fills speed up through the bar, up to 40 ms early by the end |
| Weak Foot   | `weakfoot`  | 0–100%                      | 50%         | The kick drags up to 12 ms, plays up to 40% quieter, and misses up to 25% of hits |
| Drops       | `drops`     | 0–100%                      | 50%         | Any hit can go missing, up to 15% of them |
| Flams       | `flams`     | 0–100%                      | 50%         | Up to 15% of kicks and snares get a half-velocity grace note 12–30 ms ahead |
| Flat Velocity | `flatvel` | 0–100%                      | 50%         | Pulls every hit toward one level, with up to 6% spiking to full velocity |
| Seed        | `seed`      | 0–99999                     | 1           | Selects the random pattern. Saved with the project and automatable |
| Lock Seed   | `lock`      | On, Off                     | Off         | Disables the Reroll button. Seed can still be set by hand or automation |

The Reroll button in the plugin window picks a new Seed. All params can be automated, and any of
them, such as Intensity, can be mapped to a MIDI CC with the host's controller mapping.

## Sections

The song is divided into blocks of Section Bars bars, counted from bar 0. Each block draws its section
from the seed, so a given position always gets the same section. It doesn't depend on the block before it.

| Section   | Chance                             | Kick                                | Snare           | Cymbal       |
|-----------|------------------------------------|-------------------------------------|-----------------|--------------|
| Blast     | Whatever Half-Time and Breakdown leave | As set by Blast                 | As set by Blast | Every step   |
| Half-Time | Half-Time                          | Beat 1 of every 4 quarter notes     | Beat 3 of every 4 | Every eighth |
| Breakdown | Breakdown                          | Random eighths, repeating through the block | Beat 3 of every 4 | Every quarter |
| Fill      | Fill, on the block's last bar only | About 1 step in 4                   | The other steps | First step only |

If Half-Time and Breakdown add up past 100%, they share it in proportion and no block is a plain blast.
With all three at 0%, the default, every block is a blast. Intensity thins every section, and Crash Every still applies.

## Mistakes

Turn up Amateur to hear a drummer who's still learning. Each control is multiplied by Amateur, and the
figures in the table are what it reaches with both at 100%. Mistakes come from the seed, so each one
lands in the same place on every pass. Rush only affects fills, so it needs Fill above 0%.

## Drum map

Each voice has a param with its MIDI note number, 0–127, defaulting to General MIDI drums:

| Voice | Param      | ID          | Default |
|-------|------------|-------------|---------|
| Kick  | Kick Note  | `kick`      | 36      |
| Snare | Snare Note | `snare`     | 38      |
| Hat   | Hat Note   | `hat`       | 42      |
| Ride  | Ride Note  | `ride`      | 51      |
| China | China Note | `china`     | 52      |
| Crash | Crash Note | `crashnote` | 49      |

Notes go out on MIDI channel 1 and last 30 ms.

## Build

Needs a recent Rust from [rustup](https://rustup.rs) (distro packages are usually too old).
The GUI (egui) also needs X11/GL headers:

```sh
sudo apt install libx11-xcb-dev libxcb1-dev libxcb-dri2-0-dev libxcb-icccm4-dev libxcursor-dev libxkbcommon-dev libxcb-shape0-dev libxcb-xfixes0-dev libgl-dev
```

```sh
cargo xtask bundle blastbeat --release
cargo test --lib
```

Bundles land in `target/bundled/`.

## Install (Linux)

Grab `blastbeat-<version>-linux.zip` from [Releases](https://github.com/accraze/blastbeat/releases) and unzip it, or build it yourself. Then:

```sh
cp target/bundled/Blastbeat.clap ~/.clap/
cp -r target/bundled/Blastbeat.vst3 ~/.vst3/   # optional
```

Then rescan plug-ins in your DAW.

## Releasing

Changes are listed in [CHANGELOG.md](CHANGELOG.md). See [docs/RELEASING.md](docs/RELEASING.md) for the process.

## License

Blastbeat is licensed under the [GNU General Public License v3.0 or later](LICENSE).
nih-plug itself is ISC licensed, but its VST3 bindings are GPLv3, so VST3 builds must be GPL anyway.
