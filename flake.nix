{
  description = "Development Nix flake for OpenAI Codex CLI";

  inputs = {
    nixpkgs.url = "github:meta-introspector/nixpkgs/26833ad1dad83826ef7cc52e0009ca9b7097c79f";
    nixIntrospector.url = "github:meta-introspector/flake-utils?ref=feature/CRQ-016-nixify"; # Placeholder
    rust-overlay = {
      url = "github:meta-introspector/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, nixIntrospector, rust-overlay, ... }: 
    nixIntrospector.lib.eachDefaultSystem (system:
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
