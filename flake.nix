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
        codex-rs = import ./codex-rs {
          pkgs = pkgsWithRust;
          inherit monorepo-deps;
        };
      in
      rec {
        packages = {
          codex-rs = codex-rs.package;
        };

        devShells = {
          codex-rs = codex-rs.devShell;
        };

        apps = {
          codex-rs = codex-rs.app;
        };
      }
    );
}
