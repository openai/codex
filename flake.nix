{
  description = "Development Nix flake for OpenAI Codex CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  nixConfig = {
    max-jobs = 4;
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {inherit system;};

      env = {
        PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig:$PKG_CONFIG_PATH";
      };
    in rec {
      packages = rec {
        codex-cli = pkgs.rustPlatform.buildRustPackage {
          inherit env;
          pname = "codex-cli";
          version = "0.1.0";
          cargoLock.lockFile = ./codex-rs/Cargo.lock;
          doCheck = false;
          src = ./codex-rs;
          nativeBuildInputs = with pkgs; [
            pkg-config
            openssl
          ];

          meta = with pkgs.lib; {
            description = "OpenAI Codex command‑line interface";
            license = licenses.asl20;
            homepage = "https://github.com/openai/codex";
          };
        };
        default = codex-cli;
      };
      devShells.default = pkgs.mkShell {
        inherit env;
        packages = [
          pkgs.cargo
          self.packages.${system}.codex-cli
        ];
        shellHook = ''
          ${pkgs.rustPlatform.cargoSetupHook}
        '';
      };
      apps = {
        codex = {
          type = "app";
          program = "${packages.codex-cli}/bin/codex";
        };
      };
    });
}
