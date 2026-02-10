{
  cmake,
  llvmPackages,
  openssl,
  rustPlatform,
  pkg-config,
  lib,
  ...
}:
rustPlatform.buildRustPackage (_: {
  env = {
    PKG_CONFIG_PATH = "${openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH";
  };
  pname = "codex-rs";
  version = "0.1.0";
  cargoLock.lockFile = ./Cargo.lock;
  doCheck = false;
  src = ./.;
  nativeBuildInputs = [
    cmake
    llvmPackages.clang
    llvmPackages.libclang.lib
    openssl
    pkg-config
  ];

  cargoLock.outputHashes = {
    "ratatui-0.29.0" = "sha256-HBvT5c8GsiCxMffNjJGLmHnvG77A6cqEL+1ARurBXho=";
    "crossterm-0.28.1" = "sha256-6qCtfSMuXACKFb9ATID39XyFDIEMFDmbx6SSmNe+728=";
    "nucleo-0.5.0" = "sha256-Hm4SxtTSBrcWpXrtSqeO0TACbUxq3gizg1zD/6Yw/sI=";
    "nucleo-matcher-0.3.1" = "sha256-Hm4SxtTSBrcWpXrtSqeO0TACbUxq3gizg1zD/6Yw/sI=";
    "runfiles-0.1.0" = "sha256-uJpVLcQh8wWZA3GPv9D8Nt43EOirajfDJ7eq/FB+tek=";
    "tokio-tungstenite-0.28.0" = "sha256-hJAkvWxDjB9A9GqansahWhTmj/ekcelslLUTtwqI7lw=";
    "tungstenite-0.27.0" = "sha256-AN5wql2X2yJnQ7lnDxpljNw0Jua40GtmT+w3wjER010=";
  };

  meta = with lib; {
    description = "OpenAI Codex command‑line interface rust implementation";
    license = licenses.asl20;
    homepage = "https://github.com/openai/codex";
  };
})
