# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.5](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.8.4...rmcp-v0.8.5) - 2025-11-05

### Fixed

- *(oauth)* respect oauth-protected-resource discovery ([#511](https://github.com/modelcontextprotocol/rust-sdk/pull/511))

## [0.8.4](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.8.3...rmcp-v0.8.4) - 2025-11-04

### Fixed

- *(oauth)* fix oauth credential refresh ([#509](https://github.com/modelcontextprotocol/rust-sdk/pull/509))
- do not manually construct fallback authorization metadata ([#507](https://github.com/modelcontextprotocol/rust-sdk/pull/507))
- *(doc)* add stakpak-agent to Built with rmcp section ([#500](https://github.com/modelcontextprotocol/rust-sdk/pull/500))

## [0.8.3](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.8.2...rmcp-v0.8.3) - 2025-10-22

### Fixed

- accept 204 in addition to 202 on initialize ([#497](https://github.com/modelcontextprotocol/rust-sdk/pull/497))

## [0.8.2](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.8.1...rmcp-v0.8.2) - 2025-10-21

### Added

- add type-safe elicitation schema support ([#465](https://github.com/modelcontextprotocol/rust-sdk/pull/465)) ([#466](https://github.com/modelcontextprotocol/rust-sdk/pull/466))
- *(SEP-973)* following change Icon.sizes from string to string array ([#479](https://github.com/modelcontextprotocol/rust-sdk/pull/479))

### Fixed

- *(oauth)* three oauth discovery and registration issues ([#489](https://github.com/modelcontextprotocol/rust-sdk/pull/489))
- *(oauth)* dynamic client registration should be optional ([#463](https://github.com/modelcontextprotocol/rust-sdk/pull/463))
- *(sse-client)* consume control frames; refresh message endpoint ([#448](https://github.com/modelcontextprotocol/rust-sdk/pull/448))

### Other

- Streamable HTTP: drain SSE frames until the initialize response, ignoring early notifications to prevent handshake timeouts ([#467](https://github.com/modelcontextprotocol/rust-sdk/pull/467))
- bump crate version in README.md ([#471](https://github.com/modelcontextprotocol/rust-sdk/pull/471))

## [0.8.1](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.8.0...rmcp-v0.8.1) - 2025-10-07

### Fixed

- *(oauth)* pass bearer token to all streamable http requests ([#476](https://github.com/modelcontextprotocol/rust-sdk/pull/476))
- fix spellcheck on intentional typo in CHANGELOG ([#470](https://github.com/modelcontextprotocol/rust-sdk/pull/470))

## [0.8.0](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.7.0...rmcp-v0.8.0) - 2025-10-04

### Added

- allow clients to override client_name ([#469](https://github.com/modelcontextprotocol/rust-sdk/pull/469))

### Fixed

- *(oauth)* support suffixed and prefixed well-known paths ([#459](https://github.com/modelcontextprotocol/rust-sdk/pull/459))
- generate default schema for tools with no params ([#446](https://github.com/modelcontextprotocol/rust-sdk/pull/446))

### Other

- bump to rust 1.90.0 ([#453](https://github.com/modelcontextprotocol/rust-sdk/pull/453))

## [0.7.0](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.6.4...rmcp-v0.7.0) - 2025-09-24

### Fixed

- return auth errors ([#451](https://github.com/modelcontextprotocol/rust-sdk/pull/451))
- *(oauth)* do not treat empty secret as valid for public clients ([#443](https://github.com/modelcontextprotocol/rust-sdk/pull/443))
- *(clippy)* add doc comment for generated tool attr fn ([#439](https://github.com/modelcontextprotocol/rust-sdk/pull/439))
- *(oauth)* require CSRF token as part of the OAuth authorization flow. ([#435](https://github.com/modelcontextprotocol/rust-sdk/pull/435))

### Other

- *(root)* Add Terminator to Built with rmcp section ([#437](https://github.com/modelcontextprotocol/rust-sdk/pull/437))
- Non-empty paths in OAuth2 Authorization Server Metadata URLs ([#441](https://github.com/modelcontextprotocol/rust-sdk/pull/441))

## [0.6.4](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.6.3...rmcp-v0.6.4) - 2025-09-11

### Added

- *(SEP-973)* add support for icons and websiteUrl across relevant types ([#432](https://github.com/modelcontextprotocol/rust-sdk/pull/432))
- implement context-aware completion (MCP 2025-06-18) ([#396](https://github.com/modelcontextprotocol/rust-sdk/pull/396))
- add `title` field for data types ([#410](https://github.com/modelcontextprotocol/rust-sdk/pull/410))

### Fixed

- crates/rmcp/src/handler/client/progress.rs XXXXXX -> dispatcher ([#429](https://github.com/modelcontextprotocol/rust-sdk/pull/429))
- build issue due to missing struct field ([#427](https://github.com/modelcontextprotocol/rust-sdk/pull/427))
- generate simple {} schema for tools with no parameters ([#425](https://github.com/modelcontextprotocol/rust-sdk/pull/425))

### Other

- Skip notification in initialization handshake ([#421](https://github.com/modelcontextprotocol/rust-sdk/pull/421))
- add nvim-mcp project built by rmcp ([#422](https://github.com/modelcontextprotocol/rust-sdk/pull/422))

## [0.6.3](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.6.2...rmcp-v0.6.3) - 2025-09-04

### Fixed

- change JSON-RPC request ID type from u32 to i64 ([#416](https://github.com/modelcontextprotocol/rust-sdk/pull/416))

## [0.6.2](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.6.1...rmcp-v0.6.2) - 2025-09-04

### Added

- *(rmcp)* add optional _meta to CallToolResult, EmbeddedResource, and ResourceContents ([#386](https://github.com/modelcontextprotocol/rust-sdk/pull/386))

### Fixed

- resolve compatibility issues with servers sending LSP notifications ([#413](https://github.com/modelcontextprotocol/rust-sdk/pull/413))
- remove batched json rpc  support ([#408](https://github.com/modelcontextprotocol/rust-sdk/pull/408))
- transport-streamable-http-server depends on transport-worker ([#405](https://github.com/modelcontextprotocol/rust-sdk/pull/405))
- *(typo)* correct typo in error message for transport cancellation and field. ([#404](https://github.com/modelcontextprotocol/rust-sdk/pull/404))

### Other

- Spec conformance: meta support and spec updates ([#415](https://github.com/modelcontextprotocol/rust-sdk/pull/415))
- add the rmcp-openapi and rmcp-actix-web related projects ([#406](https://github.com/modelcontextprotocol/rust-sdk/pull/406))

## [0.6.1](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.6.0...rmcp-v0.6.1) - 2025-08-29

### Added

- *(rmcp)* add authorization header support for the streamable http client ([#390](https://github.com/modelcontextprotocol/rust-sdk/pull/390))
- *(model)* add helpers to build enum from concrete values ([#393](https://github.com/modelcontextprotocol/rust-sdk/pull/393))
- *(model)* expose client method name ([#391](https://github.com/modelcontextprotocol/rust-sdk/pull/391))
- add resource_link support to tools and prompts ([#381](https://github.com/modelcontextprotocol/rust-sdk/pull/381))
- Add prompt support ([#351](https://github.com/modelcontextprotocol/rust-sdk/pull/351))
- include reqwest in transport-streamble-http-client feature ([#376](https://github.com/modelcontextprotocol/rust-sdk/pull/376))

### Fixed

- *(auth)* url parse is not correct ([#402](https://github.com/modelcontextprotocol/rust-sdk/pull/402))
- *(readme)* missing use declarations, more accurate server instructions ([#399](https://github.com/modelcontextprotocol/rust-sdk/pull/399))
- enhance transport graceful shutdown with proper writer closure ([#392](https://github.com/modelcontextprotocol/rust-sdk/pull/392))

### Other

- simplify remove_route method signature ([#401](https://github.com/modelcontextprotocol/rust-sdk/pull/401))

## [0.6.0](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.5.0...rmcp-v0.6.0) - 2025-08-19

### Added

- Add MCP Elicitation support ([#332](https://github.com/modelcontextprotocol/rust-sdk/pull/332))
- keep internal error in worker's quit reason ([#372](https://github.com/modelcontextprotocol/rust-sdk/pull/372))

### Fixed

- match shape of the calltoolresult schema ([#377](https://github.com/modelcontextprotocol/rust-sdk/pull/377))
- make stdio shutdown more graceful ([#364](https://github.com/modelcontextprotocol/rust-sdk/pull/364))
- *(tool)* remove unnecessary schema validation ([#375](https://github.com/modelcontextprotocol/rust-sdk/pull/375))
- *(rmcp)* return serialized json with structured content ([#368](https://github.com/modelcontextprotocol/rust-sdk/pull/368))

### Other

- add related project rustfs-mcp ([#378](https://github.com/modelcontextprotocol/rust-sdk/pull/378))
- *(streamable)* add document for extracting http info ([#373](https://github.com/modelcontextprotocol/rust-sdk/pull/373))

## [0.5.0](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.4.1...rmcp-v0.5.0) - 2025-08-07

### Fixed

- correct numeric types in progress notifications ([#361](https://github.com/modelcontextprotocol/rust-sdk/pull/361))

## [0.4.1](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.4.0...rmcp-v0.4.1) - 2025-08-07

### Fixed

- *(rmcp)* allow both content and structured content ([#359](https://github.com/modelcontextprotocol/rust-sdk/pull/359))

## [0.4.0](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.3.2...rmcp-v0.4.0) - 2025-08-05

### Added

- [**breaking**] Add support for `Tool.outputSchema` and `CallToolResult.structuredContent` ([#316](https://github.com/modelcontextprotocol/rust-sdk/pull/316))

### Fixed

- don't wrap errors in streamable http auth client ([#353](https://github.com/modelcontextprotocol/rust-sdk/pull/353))
- *(prompt)* remove unused code ([#343](https://github.com/modelcontextprotocol/rust-sdk/pull/343))

## [0.3.2](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.3.1...rmcp-v0.3.2) - 2025-07-30

### Fixed

- *(capabilities)* do not serialize None as null for `list_changed` ([#341](https://github.com/modelcontextprotocol/rust-sdk/pull/341))
- *(Transport)* close oneshot transport on error ([#340](https://github.com/modelcontextprotocol/rust-sdk/pull/340))
- *(oauth)* expose OAuthTokenResponse publicly ([#335](https://github.com/modelcontextprotocol/rust-sdk/pull/335))

## [0.3.1](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.3.0...rmcp-v0.3.1) - 2025-07-29

### Fixed

- use mimeType instead of mime_type for MCP specification compliance ([#339](https://github.com/modelcontextprotocol/rust-sdk/pull/339))
- return a 405 for GET and DELETE if stateful_mode=false ([#331](https://github.com/modelcontextprotocol/rust-sdk/pull/331))
- propagate tracing spans when spawning new tokio tasks ([#334](https://github.com/modelcontextprotocol/rust-sdk/pull/334))
- Explicitly added client_id as an extra parameter causes bad token requests ([#322](https://github.com/modelcontextprotocol/rust-sdk/pull/322))

### Other

- Fix formatting in crate descriptions in README.md ([#333](https://github.com/modelcontextprotocol/rust-sdk/pull/333))

## [0.3.0](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.2.1...rmcp-v0.3.0) - 2025-07-15

### Added

- unified error type ([#308](https://github.com/modelcontextprotocol/rust-sdk/pull/308))
- *(transport)* add builder & expose child stderr ([#305](https://github.com/modelcontextprotocol/rust-sdk/pull/305))

### Other

- *(deps)* update schemars requirement from 0.8 to 1.0 ([#258](https://github.com/modelcontextprotocol/rust-sdk/pull/258))
- *(rmcp)* bump rmcp-macros version to match ([#311](https://github.com/modelcontextprotocol/rust-sdk/pull/311))
- fix packages used for server example ([#309](https://github.com/modelcontextprotocol/rust-sdk/pull/309))

## [0.2.1](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.2.0...rmcp-v0.2.1) - 2025-07-03

### Other

- *(docs)* Minor README updates ([#301](https://github.com/modelcontextprotocol/rust-sdk/pull/301))

## [0.2.0](https://github.com/modelcontextprotocol/rust-sdk/compare/rmcp-v0.1.5...rmcp-v0.2.0) - 2025-07-02

### Added

- mark boxed http body as sync ([#291](https://github.com/modelcontextprotocol/rust-sdk/pull/291))
- add progress notification handling and related structures ([#282](https://github.com/modelcontextprotocol/rust-sdk/pull/282))
- allow failable service creation in streamable HTTP tower service ([#244](https://github.com/modelcontextprotocol/rust-sdk/pull/244))
- provide more context information ([#236](https://github.com/modelcontextprotocol/rust-sdk/pull/236))
- stateless mode of streamable http client ([#233](https://github.com/modelcontextprotocol/rust-sdk/pull/233))
- add cancellation_token method to `RunningService` ([#218](https://github.com/modelcontextprotocol/rust-sdk/pull/218))
- better http server support ([#199](https://github.com/modelcontextprotocol/rust-sdk/pull/199))
- throw initialize error detail ([#192](https://github.com/modelcontextprotocol/rust-sdk/pull/192))
- sse client optionally skip the endpoint event ([#187](https://github.com/modelcontextprotocol/rust-sdk/pull/187))
- *(server)* add annotation to tool macro ([#184](https://github.com/modelcontextprotocol/rust-sdk/pull/184))
- *(model)* add json schema generation support for all model types ([#176](https://github.com/modelcontextprotocol/rust-sdk/pull/176))
- *(openapi)* add OpenAPI v3 compatibility and test for nullable field schema workaround ([#135](https://github.com/modelcontextprotocol/rust-sdk/pull/135)) ([#137](https://github.com/modelcontextprotocol/rust-sdk/pull/137))
- *(extension)* extract http request part into rmcp extension ([#163](https://github.com/modelcontextprotocol/rust-sdk/pull/163))
- *(transport)* support streamable http server ([#152](https://github.com/modelcontextprotocol/rust-sdk/pull/152))
- *(oauth)* fixes + cache client credentials ([#157](https://github.com/modelcontextprotocol/rust-sdk/pull/157))
- allow use of reqwest without ring provider ([#155](https://github.com/modelcontextprotocol/rust-sdk/pull/155))
- extensions to context ([#102](https://github.com/modelcontextprotocol/rust-sdk/pull/102))
- revision-2025-03-26 without streamable http ([#84](https://github.com/modelcontextprotocol/rust-sdk/pull/84))
- *(tool)* allow tool call return a serializable value in json format ([#75](https://github.com/modelcontextprotocol/rust-sdk/pull/75)) ([#78](https://github.com/modelcontextprotocol/rust-sdk/pull/78))
- Sse server auto ping ([#74](https://github.com/modelcontextprotocol/rust-sdk/pull/74))
- *(transport)* Sse client transport trait ([#67](https://github.com/modelcontextprotocol/rust-sdk/pull/67))

### Fixed

- let users decide what to wrap in child process command ([#279](https://github.com/modelcontextprotocol/rust-sdk/pull/279))
- cancellable initialization process ([#280](https://github.com/modelcontextprotocol/rust-sdk/pull/280))
- inject part into extension when handing init req ([#275](https://github.com/modelcontextprotocol/rust-sdk/pull/275))
- streamable http server close request channel on response([#266](https://github.com/modelcontextprotocol/rust-sdk/pull/266)) ([#270](https://github.com/modelcontextprotocol/rust-sdk/pull/270))
- streamable http client close on response ([#268](https://github.com/modelcontextprotocol/rust-sdk/pull/268))
- expose TokioChildWrapper::id() in TokioChildProcess and TokioChildProcessOut ([#254](https://github.com/modelcontextprotocol/rust-sdk/pull/254))
- add compatibility handling for non-standard notifications in async_rw ([#247](https://github.com/modelcontextprotocol/rust-sdk/pull/247))
- allow SSE server router to be nested ([#240](https://github.com/modelcontextprotocol/rust-sdk/pull/240))
- error for status in post method of streamable http client ([#238](https://github.com/modelcontextprotocol/rust-sdk/pull/238))
- disable wasmbind in chrono for wasm32-unknown-unknown ([#234](https://github.com/modelcontextprotocol/rust-sdk/pull/234))
- *(examples)* add clients in examples's readme ([#225](https://github.com/modelcontextprotocol/rust-sdk/pull/225))
- generic ServerHandler ([#223](https://github.com/modelcontextprotocol/rust-sdk/pull/223))
- comment error ([#215](https://github.com/modelcontextprotocol/rust-sdk/pull/215))
- resolve the server 406 error in API calls ([#203](https://github.com/modelcontextprotocol/rust-sdk/pull/203))
- sse endpoint build follow js's `new URL(url, base)` ([#197](https://github.com/modelcontextprotocol/rust-sdk/pull/197))
- more friendly interface to get service error ([#190](https://github.com/modelcontextprotocol/rust-sdk/pull/190))
- cleanup zombie processes for child process client ([#156](https://github.com/modelcontextprotocol/rust-sdk/pull/156))
- *(schemar)* use self-defined settings ([#180](https://github.com/modelcontextprotocol/rust-sdk/pull/180))
- *(transport-sse-server)* cleanup on connection drop ([#165](https://github.com/modelcontextprotocol/rust-sdk/pull/165))
- *(test)* skip serialize tool's annotation if empty ([#160](https://github.com/modelcontextprotocol/rust-sdk/pull/160))
- fix resource leak ([#136](https://github.com/modelcontextprotocol/rust-sdk/pull/136))
- *(handler)* do call handler methods when initialize server ([#118](https://github.com/modelcontextprotocol/rust-sdk/pull/118))
- *(server)* schemars compilation errors ([#104](https://github.com/modelcontextprotocol/rust-sdk/pull/104))
- *(test)* fix test introduced by #97 ([#101](https://github.com/modelcontextprotocol/rust-sdk/pull/101))
- *(macro)* add generics marco types support ([#98](https://github.com/modelcontextprotocol/rust-sdk/pull/98))
- *(typo)* nit language corrections ([#90](https://github.com/modelcontextprotocol/rust-sdk/pull/90))
- *(typo)* s/marcos/macros/ ([#85](https://github.com/modelcontextprotocol/rust-sdk/pull/85))
- *(client)* add error enum while deal client info ([#76](https://github.com/modelcontextprotocol/rust-sdk/pull/76))
- *(notification)* fix wrongly error report in notification ([#70](https://github.com/modelcontextprotocol/rust-sdk/pull/70))
- *(test)* fix tool deserialization error ([#68](https://github.com/modelcontextprotocol/rust-sdk/pull/68))
- *(server)* add error enum while deal server info ([#51](https://github.com/modelcontextprotocol/rust-sdk/pull/51))

### Other

- add simpling example and test ([#289](https://github.com/modelcontextprotocol/rust-sdk/pull/289))
- add update for test_message_schema ([#286](https://github.com/modelcontextprotocol/rust-sdk/pull/286))
- add notion clear in model.rs ([#284](https://github.com/modelcontextprotocol/rust-sdk/pull/284))
- cov settings, and fix several building warnings ([#281](https://github.com/modelcontextprotocol/rust-sdk/pull/281))
- refactor tool macros and router implementation ([#261](https://github.com/modelcontextprotocol/rust-sdk/pull/261))
- fix regression in URL joining ([#265](https://github.com/modelcontextprotocol/rust-sdk/pull/265))
- remove erroneous definitions_path  ([#264](https://github.com/modelcontextprotocol/rust-sdk/pull/264))
- allow using a TokioCommandWrap for TokioChildProcess::new closes #243 ([#245](https://github.com/modelcontextprotocol/rust-sdk/pull/245))
- Fix typo ([#249](https://github.com/modelcontextprotocol/rust-sdk/pull/249))
- provide http server as tower service ([#228](https://github.com/modelcontextprotocol/rust-sdk/pull/228))
- *(deps)* update sse-stream requirement from 0.1.4 to 0.2.0 ([#230](https://github.com/modelcontextprotocol/rust-sdk/pull/230))
- Server info is only retrieved once during initialization ([#214](https://github.com/modelcontextprotocol/rust-sdk/pull/214))
- *(deps)* update base64 requirement from 0.21 to 0.22 ([#209](https://github.com/modelcontextprotocol/rust-sdk/pull/209))
- revert badge ([#202](https://github.com/modelcontextprotocol/rust-sdk/pull/202))
- use hierarchical readme for publishing ([#198](https://github.com/modelcontextprotocol/rust-sdk/pull/198))
- Ci/coverage badge ([#191](https://github.com/modelcontextprotocol/rust-sdk/pull/191))
- fix error introduced by merge, and reorganize feature ([#185](https://github.com/modelcontextprotocol/rust-sdk/pull/185))
- Transport trait and worker transport, and streamable http client with those new features. ([#167](https://github.com/modelcontextprotocol/rust-sdk/pull/167))
- add oauth2 support ([#130](https://github.com/modelcontextprotocol/rust-sdk/pull/130))
- remove un-used tower.rs ([#125](https://github.com/modelcontextprotocol/rust-sdk/pull/125))
- update calculator example description ([#115](https://github.com/modelcontextprotocol/rust-sdk/pull/115))
- fix the url ([#120](https://github.com/modelcontextprotocol/rust-sdk/pull/120))
- add a simple chat client for example ([#119](https://github.com/modelcontextprotocol/rust-sdk/pull/119))
- add an overview to `rmcp/src/lib.rs` ([#116](https://github.com/modelcontextprotocol/rust-sdk/pull/116))
- *(context)* test context request handling and refactor for reusable client-server tests ([#97](https://github.com/modelcontextprotocol/rust-sdk/pull/97))
- *(logging)* Add tests for logging  ([#96](https://github.com/modelcontextprotocol/rust-sdk/pull/96))
- Adopt Devcontainer for Development Environment ([#81](https://github.com/modelcontextprotocol/rust-sdk/pull/81))
- fix typos ([#79](https://github.com/modelcontextprotocol/rust-sdk/pull/79))
- format and fix typo ([#72](https://github.com/modelcontextprotocol/rust-sdk/pull/72))
- add documentation generation job ([#59](https://github.com/modelcontextprotocol/rust-sdk/pull/59))
- add test with js server ([#65](https://github.com/modelcontextprotocol/rust-sdk/pull/65))
- fmt the project ([#54](https://github.com/modelcontextprotocol/rust-sdk/pull/54))
- *(sse_server)* separate router and server startup ([#52](https://github.com/modelcontextprotocol/rust-sdk/pull/52))
- fix broken link ([#53](https://github.com/modelcontextprotocol/rust-sdk/pull/53))
- fix the branch name for git dependency ([#46](https://github.com/modelcontextprotocol/rust-sdk/pull/46))
- Move whole rmcp crate to official rust sdk ([#44](https://github.com/modelcontextprotocol/rust-sdk/pull/44))
- Initial commit
