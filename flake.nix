{
  description = "Development Nix flake for OpenAI Codex CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
      in
      rec {
        packages = {
          codex-rs = pkgs.callPackage ./codex-rs/derivation.nix { };
        };
        apps = {
          codex-rs = {
            type = "app";
            program = "${packages.codex-rs}/bin/codex";
          };
        };
        defaultPackage = packages.codex-rs;
        defaultApp = apps.codex-rs;
        formatter = pkgs.nixfmt-tree;
      }
    );
}
