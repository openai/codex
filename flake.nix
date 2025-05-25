{
  description = "Development Nix flake for OpenAI Codex CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, flake-utils, rust-overlay, ... }: 
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };
        pkgsWithRust = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        monorepo-deps = with pkgs; [
          # for precommit hook
          pnpm
          husky
        ];
        codex-cli = import ./codex-cli {
          inherit pkgs monorepo-deps;
        };
        codex-rs = import ./codex-rs {
          pkgs = pkgsWithRust;
          inherit monorepo-deps;
        };
      in
      rec {
        packages = {
          codex-cli = codex-cli.package;
          codex-rs = codex-rs.package;
        };

        devShells = {
          codex-cli = codex-cli.devShell;
          codex-rs = codex-rs.devShell;
        };

        apps = {
          codex-cli = codex-cli.app;
          codex-rs = codex-rs.app;
        };

        defaultPackage = packages.codex-cli;
        defaultApp = apps.codex-cli;
        defaultDevShell = devShells.codex-cli;
      }
    );
}
