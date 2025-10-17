{
  openssl,
  rustPlatform,
  pkg-config,
  lib,
  ...
}:
rustPlatform.buildRustPackage (finalAttrs: {
  env = {
    PKG_CONFIG_PATH = "${openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH";
  };
  pname = "codex-rs";
  version = "0.1.0";
  cargoLock.lockFile = ./Cargo.lock;
  doCheck = false;
  src = ./.;
  nativeBuildInputs = [
    pkg-config
    openssl
  ];

  cargoLock.outputHashes = {
    "ratatui-0.29.0" = "sha256-HBvT5c8GsiCxMffNjJGLmHnvG77A6cqEL+1ARurBXho=";
  };

  meta = with lib; {
    description = "OpenAI Codex command‑line interface rust implementation";
    license = licenses.asl20;
    homepage = "https://github.com/openai/codex";
  };
})
