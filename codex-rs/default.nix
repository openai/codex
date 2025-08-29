{ pkgs, monorep-deps ? [], ... }:
let
  env = {
    PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH";
  };
  rustTools = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
  rustPlatform = pkgs.makeRustPlatform {
    cargo = rustTools;
    rustc = rustTools;
  };
in
rec {
  package = rustPlatform.buildRustPackage {
    inherit env;
    pname = "codex-rs";
    version = "0.1.0";
    cargoLock = {
      lockFile = ./Cargo.lock;
      allowBuiltinFetchGit = true;
    };
    doCheck = false;
    src = ./.;
    nativeBuildInputs = with pkgs; [
      pkg-config
      openssl
    ];
    meta = with pkgs.lib; {
      description = "OpenAI Codex command‑line interface rust implementation";
      license = licenses.asl20;
      homepage = "https://github.com/openai/codex";
    };
  };
  devShell = pkgs.mkShell {
    inherit env;
    name = "codex-rs-dev";
    packages = monorep-deps ++ [
      rustTools
      package
    ];
    shellHook = ''
      echo "Entering development shell for codex-rs"
      alias codex="cd ${package.src}/tui; cargo run; cd -"
      ${rustPlatform.cargoSetupHook}
    '';
  };
  app = {
    type = "app";
    program = "${package}/bin/codex";
  };
}
