# Changelog

All notable changes to Blastbeat are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.3.0] - 2026-11-06

### Added
- Amateur layer: rushed fills, a weak kick foot, dropped hits, flams and flat velocity with occasional
  spikes, each with its own control under an Amateur macro. It defaults to 0%, so existing projects are unchanged.

## [0.2.0] - 2026-11-06

### Added
- Sections: blocks of Section Bars bars that can become half-time or a breakdown, or end in a fill,
  set by Half-Time, Breakdown and Fill chances. All default to 0%, so existing projects are unchanged.

## [0.1.0] - 2026-11-06

### Added
- Stateless blast-beat generator: traditional and hammer blasts at 1/8, 1/16, 1/16T and 1/32.
- Cymbal layer (hat, ride, china) with an accent on beat 1, and a crash every N bars.
- Configurable drum map, defaulting to General MIDI.
- Feel: Jitter, Drift and Drift Rate, scaled by the Human macro.
- Seed with a Reroll button and a lock, and an Intensity macro.

[Unreleased]: https://github.com/accraze/blastbeat/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/accraze/blastbeat/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/accraze/blastbeat/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/accraze/blastbeat/releases/tag/v0.1.0
