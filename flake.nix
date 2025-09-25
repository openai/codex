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
        pkgsWithRust = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        codex-rs = import ./codex-rs {
          pkgs = pkgsWithRust;
        };
      in
      {
        packages = {
          codex-rs = codex-rs.package;
          default = codex-rs.package;
        };

        devShells = {
          codex-rs = codex-rs.devShell;
          default = codex-rs.devShell;
        };

        apps = {
          codex-rs = codex-rs.app;
          default = codex-rs.app;
        };
      }
    );
}
