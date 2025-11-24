# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.4](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-macros-v0.8.3...rmcp-macros-v0.8.4) - 2025-11-04

### Fixed

- *(doc)* add stakpak-agent to Built with rmcp section ([#500](https://github.com/modelcontextprotocol/rust-sdk/pull/500))

## [0.8.2](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-macros-v0.8.1...rmcp-macros-v0.8.2) - 2025-10-21

### Other

- *(macro)* fix visibility attribute's usage of handler macro ([#481](https://github.com/modelcontextprotocol/rust-sdk/pull/481))
- bump crate version in README.md ([#471](https://github.com/modelcontextprotocol/rust-sdk/pull/471))

## [0.8.0](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-macros-v0.7.0...rmcp-macros-v0.8.0) - 2025-10-04

### Fixed

- generate default schema for tools with no params ([#446](https://github.com/modelcontextprotocol/rust-sdk/pull/446))

## [0.7.0](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-macros-v0.6.4...rmcp-macros-v0.7.0) - 2025-09-24

### Fixed

- *(macros)* support #[doc = include_str!(...)] for macros ([#444](https://github.com/modelcontextprotocol/rust-sdk/pull/444))
- *(clippy)* add doc comment for generated tool attr fn ([#439](https://github.com/modelcontextprotocol/rust-sdk/pull/439))

### Other

- *(root)* Add Terminator to Built with rmcp section ([#437](https://github.com/modelcontextprotocol/rust-sdk/pull/437))

## [0.6.4](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-macros-v0.6.3...rmcp-macros-v0.6.4) - 2025-09-11

### Added

- *(SEP-973)* add support for icons and websiteUrl across relevant types ([#432](https://github.com/modelcontextprotocol/rust-sdk/pull/432))
- add `title` field for data types ([#410](https://github.com/modelcontextprotocol/rust-sdk/pull/410))

### Fixed

- generate simple {} schema for tools with no parameters ([#425](https://github.com/modelcontextprotocol/rust-sdk/pull/425))

### Other

- add nvim-mcp project built by rmcp ([#422](https://github.com/modelcontextprotocol/rust-sdk/pull/422))

## [0.6.2](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-macros-v0.6.1...rmcp-macros-v0.6.2) - 2025-09-04

### Fixed

- *(typo)* correct typo in error message for transport cancellation and field. ([#404](https://github.com/modelcontextprotocol/rust-sdk/pull/404))

### Other

- add the rmcp-openapi and rmcp-actix-web related projects ([#406](https://github.com/modelcontextprotocol/rust-sdk/pull/406))

## [0.6.1](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-macros-v0.6.0...rmcp-macros-v0.6.1) - 2025-08-29

### Added

- Add prompt support ([#351](https://github.com/modelcontextprotocol/rust-sdk/pull/351))

### Fixed

- *(macros)* Allow macros to work even if Future is not in scope ([#385](https://github.com/modelcontextprotocol/rust-sdk/pull/385))

## [0.6.0](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-macros-v0.5.0...rmcp-macros-v0.6.0) - 2025-08-19

### Other

- add related project rustfs-mcp ([#378](https://github.com/modelcontextprotocol/rust-sdk/pull/378))

## [0.4.0](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-macros-v0.3.2...rmcp-macros-v0.4.0) - 2025-08-05

### Added

- [**breaking**] Add support for `Tool.outputSchema` and `CallToolResult.structuredContent` ([#316](https://github.com/modelcontextprotocol/rust-sdk/pull/316))

### Other

- README.md codeblock terminator ([#348](https://github.com/modelcontextprotocol/rust-sdk/pull/348))

## [0.3.1](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-macros-v0.3.0...rmcp-macros-v0.3.1) - 2025-07-29

### Other

- Fix formatting in crate descriptions in README.md ([#333](https://github.com/modelcontextprotocol/rust-sdk/pull/333))

## [0.3.0](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-macros-v0.2.1...rmcp-macros-v0.3.0) - 2025-07-15

### Added

- unified error type ([#308](https://github.com/modelcontextprotocol/rust-sdk/pull/308))

### Other

- *(deps)* update darling requirement from 0.20 to 0.21 ([#318](https://github.com/modelcontextprotocol/rust-sdk/pull/318))

## [0.2.1](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-macros-v0.2.0...rmcp-macros-v0.2.1) - 2025-07-03

### Other

- *(docs)* Minor README updates ([#301](https://github.com/modelcontextprotocol/rust-sdk/pull/301))

## [0.2.0](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-macros-v0.1.5...rmcp-macros-v0.2.0) - 2025-07-02

### Added

- add progress notification handling and related structures ([#282](https://github.com/modelcontextprotocol/rust-sdk/pull/282))
- *(server)* add annotation to tool macro ([#184](https://github.com/modelcontextprotocol/rust-sdk/pull/184))
- *(model)* add json schema generation support for all model types ([#176](https://github.com/modelcontextprotocol/rust-sdk/pull/176))
- *(transport)* support streamable http server ([#152](https://github.com/modelcontextprotocol/rust-sdk/pull/152))
- *(rmcp-macro)* generate description from docs ([#141](https://github.com/modelcontextprotocol/rust-sdk/pull/141))
- revision-2025-03-26 without streamable http ([#84](https://github.com/modelcontextprotocol/rust-sdk/pull/84))

### Fixed

- *(examples)* add clients in examples's readme ([#225](https://github.com/modelcontextprotocol/rust-sdk/pull/225))
- generic ServerHandler ([#223](https://github.com/modelcontextprotocol/rust-sdk/pull/223))
- cleanup zombie processes for child process client ([#156](https://github.com/modelcontextprotocol/rust-sdk/pull/156))
- *(rmcp-macros)* fix extract_doc_line code ([#142](https://github.com/modelcontextprotocol/rust-sdk/pull/142))
- *(macros)* add error deal ([#109](https://github.com/modelcontextprotocol/rust-sdk/pull/109))
- *(macro)* add generics marco types support ([#98](https://github.com/modelcontextprotocol/rust-sdk/pull/98))
- *(typo)* s/marcos/macros/ ([#85](https://github.com/modelcontextprotocol/rust-sdk/pull/85))
- *(test)* fix tool deserialization error ([#68](https://github.com/modelcontextprotocol/rust-sdk/pull/68))

### Other

- refactor tool macros and router implementation ([#261](https://github.com/modelcontextprotocol/rust-sdk/pull/261))
- revert badge ([#202](https://github.com/modelcontextprotocol/rust-sdk/pull/202))
- use hierarchical readme for publishing ([#198](https://github.com/modelcontextprotocol/rust-sdk/pull/198))
- Ci/coverage badge ([#191](https://github.com/modelcontextprotocol/rust-sdk/pull/191))
- Transport trait and worker transport, and streamable http client with those new features. ([#167](https://github.com/modelcontextprotocol/rust-sdk/pull/167))
- add oauth2 support ([#130](https://github.com/modelcontextprotocol/rust-sdk/pull/130))
- update calculator example description ([#115](https://github.com/modelcontextprotocol/rust-sdk/pull/115))
- fix the url ([#120](https://github.com/modelcontextprotocol/rust-sdk/pull/120))
- add a simple chat client for example ([#119](https://github.com/modelcontextprotocol/rust-sdk/pull/119))
- add spell check ([#82](https://github.com/modelcontextprotocol/rust-sdk/pull/82))
- Adopt Devcontainer for Development Environment ([#81](https://github.com/modelcontextprotocol/rust-sdk/pull/81))
- fix typos ([#79](https://github.com/modelcontextprotocol/rust-sdk/pull/79))
- format and fix typo ([#72](https://github.com/modelcontextprotocol/rust-sdk/pull/72))
- add documentation generation job ([#59](https://github.com/modelcontextprotocol/rust-sdk/pull/59))
- fmt the project ([#54](https://github.com/modelcontextprotocol/rust-sdk/pull/54))
- fix broken link ([#53](https://github.com/modelcontextprotocol/rust-sdk/pull/53))
- fix the branch name for git dependency ([#46](https://github.com/modelcontextprotocol/rust-sdk/pull/46))
- Move whole rmcp crate to official rust sdk ([#44](https://github.com/modelcontextprotocol/rust-sdk/pull/44))
- Initial commit
