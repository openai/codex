{
  cmake,
  fetchurl,
  fetchzip,
  llvmPackages,
  openssl,
  libcap ? null,
  rustPlatform,
  pkg-config,
  lib,
  stdenv,
  version ? "0.0.0",
  ...
}:
let
  # rusty_v8 tries to download a prebuilt static library at build time, which
  # Nix's sandbox blocks. Pre-fetch it and point the build at it via
  # RUSTY_V8_ARCHIVE so no network access is needed during the build.
  rustyV8Version = "146.4.0";
  # webrtc-sys also downloads a prebuilt WebRTC binary at build time. Pre-fetch
  # the zip (fetchzip auto-extracts it) and point the build at it via
  # LK_CUSTOM_WEBRTC so no network access is needed during the build.
  webrtcTag = "webrtc-24f6822-2";
  libwebrtc = fetchzip (
    {
      "x86_64-linux" = {
        url = "https://github.com/livekit/rust-sdks/releases/download/${webrtcTag}/webrtc-linux-x64-release.zip";
        hash = "sha256-ylzMlvNF4KV/PbnjBxVRA6YmNenwS314bMO9XRaYrmE=";
      };
      "aarch64-linux" = {
        url = "https://github.com/livekit/rust-sdks/releases/download/${webrtcTag}/webrtc-linux-arm64-release.zip";
        hash = "sha256-T+Nz6dUQF/5AENdrPk5DM3pc+dNBCzahmKAeQANOLTQ=";
      };
      "x86_64-darwin" = {
        url = "https://github.com/livekit/rust-sdks/releases/download/${webrtcTag}/webrtc-mac-x64-release.zip";
        hash = "sha256-XapngujlXtcDEGd2hacmP3nHFycEVZRybO/ORHPc6Og=";
      };
      "aarch64-darwin" = {
        url = "https://github.com/livekit/rust-sdks/releases/download/${webrtcTag}/webrtc-mac-arm64-release.zip";
        hash = "sha256-4IwJM6EzTFgQd2AdX+Hj9NWzmyqXrSioRax2L6GKL1U=";
      };
    }
    .${stdenv.hostPlatform.system}
  );
  librusty_v8 = fetchurl (
    {
      "x86_64-linux" = {
        url = "https://github.com/denoland/rusty_v8/releases/download/v${rustyV8Version}/librusty_v8_release_x86_64-unknown-linux-gnu.a.gz";
        hash = "sha256-5ktNmeSuKTouhGJEqJuAF4uhA4LBP7WRwfppaPUpEVM=";
      };
      "aarch64-linux" = {
        url = "https://github.com/denoland/rusty_v8/releases/download/v${rustyV8Version}/librusty_v8_release_aarch64-unknown-linux-gnu.a.gz";
        hash = "sha256-2/FlsHyBvbBUvARrQ9I+afz3vMGkwbW0d2mDpxBi7Ng=";
      };
      "x86_64-darwin" = {
        url = "https://github.com/denoland/rusty_v8/releases/download/v${rustyV8Version}/librusty_v8_release_x86_64-apple-darwin.a.gz";
        hash = "sha256-YwzSQPG77NsHFBfcGDh6uBz2fFScHFFaC0/Pnrpke7c=";
      };
      "aarch64-darwin" = {
        url = "https://github.com/denoland/rusty_v8/releases/download/v${rustyV8Version}/librusty_v8_release_aarch64-apple-darwin.a.gz";
        hash = "sha256-v+LJvjKlbChUbw+WWCXuaPv2BkBfMQzE4XtEilaM+Yo=";
      };
    }
    .${stdenv.hostPlatform.system}
  );
in
rustPlatform.buildRustPackage (_: {
  env.PKG_CONFIG_PATH = lib.makeSearchPathOutput "dev" "lib/pkgconfig" (
    [ openssl ] ++ lib.optionals stdenv.isLinux [ libcap ]
  );
  env.RUSTY_V8_ARCHIVE = librusty_v8;
  env.LK_CUSTOM_WEBRTC = libwebrtc;
  # fat-LTO with v8 requires several GB per binary link job; running them in
  # parallel exhausts RAM.  Serialise cargo jobs to one at a time.
  # (lto="thin" or disabling LTO entirely would also work but changes binary
  # characteristics; limiting parallelism is the minimal-impact fix.)
  env.CARGO_BUILD_JOBS = "1";
  pname = "codex-rs";
  inherit version;
  cargoLock.lockFile = ./Cargo.lock;
  doCheck = false;
  src = ./.;

  # Patch the workspace Cargo.toml so that cargo embeds the correct version in
  # CARGO_PKG_VERSION (which the binary reads via env!("CARGO_PKG_VERSION")).
  # On release commits the Cargo.toml already contains the real version and
  # this sed is a no-op.
  postPatch = ''
    sed -i 's/^version = "0\.0\.0"$/version = "${version}"/' Cargo.toml
  '';
  nativeBuildInputs = [
    cmake
    llvmPackages.clang
    llvmPackages.libclang.lib
    openssl
    pkg-config
  ] ++ lib.optionals stdenv.isLinux [
    libcap
  ];

  cargoLock.outputHashes = {
    "ratatui-0.29.0" = "sha256-HBvT5c8GsiCxMffNjJGLmHnvG77A6cqEL+1ARurBXho=";
    "crossterm-0.28.1" = "sha256-6qCtfSMuXACKFb9ATID39XyFDIEMFDmbx6SSmNe+728=";
    "nucleo-0.5.0" = "sha256-Hm4SxtTSBrcWpXrtSqeO0TACbUxq3gizg1zD/6Yw/sI=";
    "nucleo-matcher-0.3.1" = "sha256-Hm4SxtTSBrcWpXrtSqeO0TACbUxq3gizg1zD/6Yw/sI=";
    "runfiles-0.1.0" = "sha256-uJpVLcQh8wWZA3GPv9D8Nt43EOirajfDJ7eq/FB+tek=";
    "tokio-tungstenite-0.28.0" = "sha256-hJAkvWxDjB9A9GqansahWhTmj/ekcelslLUTtwqI7lw=";
    "tungstenite-0.27.0" = "sha256-AN5wql2X2yJnQ7lnDxpljNw0Jua40GtmT+w3wjER010=";
    "libwebrtc-0.3.26" = "sha256-0HPuwaGcqpuG+Pp6z79bCuDu/DyE858VZSYr3DKZD9o=";
  };

  meta = with lib; {
    description = "OpenAI Codex command‑line interface rust implementation";
    license = licenses.asl20;
    homepage = "https://github.com/openai/codex";
    mainProgram = "codex";
  };
})
