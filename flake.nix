{
  description = "Development Nix flake for OpenAI Codex CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        codex-rs = nixpkgs.legacyPackages.${system} ./codex-rs { };
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
