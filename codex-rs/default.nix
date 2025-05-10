{ pkgs, monorep-deps ? [], ... }:
let
  env = {
    PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH";
  };
in
rec {
  package = pkgs.rustPlatform.buildRustPackage {
    inherit env;
    pname = "codex-rs";
    version = "0.1.0";
    cargoLock.lockFile = ./Cargo.lock;
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
      pkgs.cargo
      package
    ];
    shellHook = ''
      echo "Entering development shell for codex-rs"
      alias codex="cd ${package.src}/tui; cargo run; cd -"
      ${pkgs.rustPlatform.cargoSetupHook}
    '';
  };
  app = {
    type = "app";
    program = "${package}/bin/codex";
  };
}
