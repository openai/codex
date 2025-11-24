# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0](https://github.com/ratatui/ratatui-macros/compare/v0.5.0...v0.6.0) - 2024-10-21

### Other

- *(deps)* bump the cargo-dependencies group with 2 updates ([#73](https://github.com/ratatui/ratatui-macros/pull/73))
- *(deps)* bump ratatui from 0.28.0 to 0.28.1 in the cargo-dependencies group ([#70](https://github.com/ratatui/ratatui-macros/pull/70))

## [0.5.0] - 2024-08-12

### 🐛 Bug Fixes

- Bump version to 0.5.0

## [0.4.4](https://github.com/ratatui-org/ratatui-macros/compare/v0.4.3...v0.4.4) - 2024-08-09

### Other

- *(deps)* bump ratatui to 0.28.0 ([#66](https://github.com/ratatui-org/ratatui-macros/pull/66))
- *(deps)* bump trybuild from 1.0.98 to 1.0.99 in the cargo-dependencies group ([#65](https://github.com/ratatui-org/ratatui-macros/pull/65))
- *(deps)* bump trybuild from 1.0.97 to 1.0.98 in the cargo-dependencies group ([#62](https://github.com/ratatui-org/ratatui-macros/pull/62))

## [0.4.3](https://github.com/ratatui-org/ratatui-macros/compare/v0.4.2...v0.4.3) - 2024-07-22

### Added

- allow span macro to accept a bare expression ([#61](https://github.com/ratatui-org/ratatui-macros/pull/61))

### Other

- *(deps)* bump trybuild from 1.0.96 to 1.0.97 in the cargo-dependencies group ([#59](https://github.com/ratatui-org/ratatui-macros/pull/59))

## [0.4.2](https://github.com/ratatui-org/ratatui-macros/compare/v0.4.1...v0.4.2) - 2024-06-29

### Added

- Use `::ratatui` instead of `ratatui` ([#54](https://github.com/ratatui-org/ratatui-macros/pull/54))
- Add row! macro ([#52](https://github.com/ratatui-org/ratatui-macros/pull/52))

### Other

- Update README with row! documentation ([#56](https://github.com/ratatui-org/ratatui-macros/pull/56))
- Make doc examples shorter by removing duplicate imports ([#55](https://github.com/ratatui-org/ratatui-macros/pull/55))

## [0.4.1](https://github.com/ratatui-org/ratatui-macros/compare/v0.4.0...v0.4.1) - 2024-06-24

### Other

- *(deps)* bump ratatui from 0.26.3 to 0.27.0 in the cargo-dependencies group ([#51](https://github.com/ratatui-org/ratatui-macros/pull/51))
- Update dependabot.yml to group dependencies ([#50](https://github.com/ratatui-org/ratatui-macros/pull/50))
- *(deps)* bump ratatui from 0.26.2 to 0.26.3 ([#48](https://github.com/ratatui-org/ratatui-macros/pull/48))
- *(deps)* bump trybuild from 1.0.95 to 1.0.96 ([#47](https://github.com/ratatui-org/ratatui-macros/pull/47))

## [0.4.0](https://github.com/ratatui-org/ratatui-macros/compare/v0.3.1...v0.4.0) - 2024-05-15

### Added

- *(layout)* [**breaking**] Use `*=` instead of `=*` ([#45](https://github.com/ratatui-org/ratatui-macros/pull/45))

## [0.3.1](https://github.com/ratatui-org/ratatui-macros/compare/v0.3.0...v0.3.1) - 2024-05-13

### Added

- Better error messages for `span!` macro ([#43](https://github.com/ratatui-org/ratatui-macros/pull/43))

### Fixed

- downgrade ratatui to 0.26.2 ([#41](https://github.com/ratatui-org/ratatui-macros/pull/41))

### Other

- Update authors to ratatui developers ([#44](https://github.com/ratatui-org/ratatui-macros/pull/44))

## [0.3.0](https://github.com/ratatui-org/ratatui-macros/compare/v0.2.4...v0.3.0) - 2024-05-09

### Added

- Use release-plz ([#38](https://github.com/ratatui-org/ratatui-macros/pull/38))
- Add text! macro ([#36](https://github.com/ratatui-org/ratatui-macros/pull/36))
- Add fill constraint ([#34](https://github.com/ratatui-org/ratatui-macros/pull/34))
- [**breaking**] Remove color `palette!` macro ([#32](https://github.com/ratatui-org/ratatui-macros/pull/32))
- Replace raw! and styled! with span! macro ([#30](https://github.com/ratatui-org/ratatui-macros/pull/30))
- Add `line!` attribute macro ([#29](https://github.com/ratatui-org/ratatui-macros/pull/29))
- *(text)* add raw! and styled! macros ([#4](https://github.com/ratatui-org/ratatui-macros/pull/4))

### Fixed

- Update repo url in Cargo.toml ([#39](https://github.com/ratatui-org/ratatui-macros/pull/39))

### Other

- Use `.areas(area)` instead of `.split(area).to_vec().try_into().unwrap()` ([#37](https://github.com/ratatui-org/ratatui-macros/pull/37))
- Update README.md with short description of span and line macros ([#33](https://github.com/ratatui-org/ratatui-macros/pull/33))
- format using cargo +nightly fmt ([#31](https://github.com/ratatui-org/ratatui-macros/pull/31))
- *(deps)* bump ratatui from 0.27.0-alpha.3 to 0.27.0-alpha.5 ([#27](https://github.com/ratatui-org/ratatui-macros/pull/27))
- *(deps)* bump trybuild from 1.0.91 to 1.0.93 ([#28](https://github.com/ratatui-org/ratatui-macros/pull/28))
- *(deps)* bump ratatui from 0.27.0-alpha.2 to 0.27.0-alpha.3 ([#24](https://github.com/ratatui-org/ratatui-macros/pull/24))
- *(deps)* bump trybuild from 1.0.90 to 1.0.91 ([#23](https://github.com/ratatui-org/ratatui-macros/pull/23))
- *(deps)* bump trybuild from 1.0.88 to 1.0.90 ([#20](https://github.com/ratatui-org/ratatui-macros/pull/20))
- *(deps)* bump ratatui from 0.27.0-alpha.0 to 0.27.0-alpha.2 ([#22](https://github.com/ratatui-org/ratatui-macros/pull/22))
- *(deps)* bump mio from 0.8.10 to 0.8.11 ([#18](https://github.com/ratatui-org/ratatui-macros/pull/18))
- *(deps)* bump ratatui from 0.26.0-alpha.1 to 0.27.0-alpha.0 ([#19](https://github.com/ratatui-org/ratatui-macros/pull/19))
- add cargo husky pre-commit hook ([#8](https://github.com/ratatui-org/ratatui-macros/pull/8))
- Create dependabot.yml ([#7](https://github.com/ratatui-org/ratatui-macros/pull/7))
- use rust cache to cache deps ([#6](https://github.com/ratatui-org/ratatui-macros/pull/6))
- readme tweaks ([#5](https://github.com/ratatui-org/ratatui-macros/pull/5))
- Update README.md
- Update README.md
- Update README.md
- Update README.md
- Add link to ratatui
