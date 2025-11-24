{
  description = "A simple rust flake using rust-overlay and craneLib";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    nix-github-actions = {
      url = "github:nix-community/nix-github-actions";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = {
    self,
    crane,
    flake-utils,
    nixpkgs,
    rust-overlay,
    advisory-db,
    nix-github-actions,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            rust-overlay.overlays.default
          ];
        };
        inherit (pkgs) lib;

        stableToolchain = pkgs.rust-bin.stable.latest.default;
        stableToolchainWithLLvmTools = pkgs.rust-bin.stable.latest.default.override {
          extensions = ["rust-src" "llvm-tools"];
        };
        stableToolchainWithRustAnalyzer = pkgs.rust-bin.stable.latest.default.override {
          extensions = ["rust-src" "rust-analyzer"];
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain stableToolchain;
        craneLibLLvmTools = (crane.mkLib pkgs).overrideToolchain stableToolchainWithLLvmTools;

        sourceFilters = path: type: (craneLib.filterCargoSources path type) || (lib.hasSuffix ".ascii" path);
        src = lib.cleanSourceWith {
          filter = sourceFilters;
          src = ./.;
        };
        commonArgs =
          {
            inherit src;
            pname = "ansi-to-tui";
            doCheck = false;
            # LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            # nativeBuildInputs = with pkgs; [
            #   cmake
            #   llvmPackages.libclang.lib
            # ];
            buildInputs = with pkgs;
              []
              ++ (lib.optionals pkgs.stdenv.isDarwin [
                libiconv
                # darwin.apple_sdk.frameworks.Metal
              ]);
          }
          // (lib.optionalAttrs pkgs.stdenv.isLinux {
            # BINDGEN_EXTRA_CLANG_ARGS = "-I${pkgs.llvmPackages.libclang.lib}/lib/clang/18/include";
          });
        cargoArtifacts = craneLib.buildPackage commonArgs;
      in {
        checks =
          {
            ansi-to-tui-clippy = craneLib.cargoClippy (commonArgs
              // {
                inherit cargoArtifacts;
                cargoClippyExtraArgs = "--all-targets -- --deny warnings";
              });
            ansi-to-tui-docs = craneLib.cargoDoc (commonArgs // {inherit cargoArtifacts;});
            ansi-to-tui-fmt = craneLib.cargoFmt {inherit src;};
            ansi-to-tui-toml-fmt = craneLib.taploFmt {
              src = pkgs.lib.sources.sourceFilesBySuffices src [".toml"];
            };
            # Audit dependencies
            ansi-to-tui-audit = craneLib.cargoAudit {
              inherit src advisory-db;
            };

            # Audit licenses
            ansi-to-tui-deny = craneLib.cargoDeny {
              inherit src;
            };
            ansi-to-tui-nextest = craneLib.cargoNextest (commonArgs
              // {
                inherit cargoArtifacts;
                partitions = 1;
                partitionType = "count";
              });
          }
          // lib.optionalAttrs (!pkgs.stdenv.isDarwin) {
            ansi-to-tui-llvm-coverage = craneLibLLvmTools.cargoLlvmCov (commonArgs // {inherit cargoArtifacts;});
          };

        packages = rec {
          ansi-to-tui = craneLib.buildPackage (commonArgs // {inherit cargoArtifacts;});
          default = ansi-to-tui;
        };

        devShells = {
          default = pkgs.mkShell {
            packages = with pkgs; [
              stableToolchainWithRustAnalyzer
              cargo-nextest
              cargo-deny
              cargo-outdated
              cargo-semver-checks
            ];
          };
        };
      }
    )
    // {
      githubActions = nix-github-actions.lib.mkGithubMatrix {
        checks = nixpkgs.lib.getAttrs ["x86_64-linux"] self.checks;
      };
    };
}
