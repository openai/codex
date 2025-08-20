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

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      # Base pkgs with the Rust overlay
      pkgsBase = import nixpkgs {
        inherit system;
        overlays = [rust-overlay.overlays.default];
      };

      # Use toolchain pinned in ./codex-rs/rust-toolchain.toml, fallback to stable 1.89.0
      rustToolchain =
        if builtins.pathExists ./codex-rs/rust-toolchain.toml
        then pkgsBase.rust-bin.fromRustupToolchainFile ./codex-rs/rust-toolchain.toml
        else pkgsBase.rust-bin.stable."1.89.0".default;

      # Reimport pkgs and override rustPlatform to use that toolchain for builds
      pkgs = import nixpkgs {
        inherit system;
        overlays = [
          rust-overlay.overlays.default
          (final: prev: {
            rustPlatform = prev.makeRustPlatform {
              cargo = rustToolchain;
              rustc = rustToolchain;
            };
          })
        ];
      };

      monorepo-deps = with pkgs; [
        pnpm
        husky
      ];

      codex-rs = import ./codex-rs {
        inherit pkgs monorepo-deps;
      };
    in rec {
      packages = {
        codex-rs = codex-rs.package;
        default = codex-rs.package;
      };

      apps = {
        codex-rs = codex-rs.app;
        default = codex-rs.app;
      };

      devShells = {
        # Dev shell includes the pinned toolchain on PATH
        codex-rs = pkgs.mkShell {
          packages = monorepo-deps ++ [rustToolchain codex-rs.package];
        };
        default = devShells.codex-rs;
      };

      # Optional CI hook to ensure the package builds
      checks = {
        codex-rs-build = packages.codex-rs;
      };
    });
}
