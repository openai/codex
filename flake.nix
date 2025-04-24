{
  description = "Development Nix flake for OpenAI Codex CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    nixpkgs,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {inherit system;};
      node = pkgs.nodejs_22;
      pnpm = pkgs.pnpm_10;
      version = "0.1.0";
      hash = "";

      buildPnpmPackageFn = {
        pkgs,
        lib,
        ...
      }: args @ {
        # to derive pname and version
        packageJsonPath,
        # to provision sources additional to monorepo boilerplate
        extraSrcs,
        # workspace project names required for build (e.g. anything with "workspace:*" verison declaration)
        pnpmWorkspaces ? [],
        hash ? lib.fakeHash,
        pnpm ? pnpm,
        ...
      }: let
        src = with lib.fileset; (toSource {
          root = ./.;
          fileset = unions (
            [
              ./package.json
              ./pnpm-lock.yaml
              ./pnpm-workspace.yaml
            ]
            ++ extraSrcs
          );
        });
        packageJson = lib.importJSON packageJsonPath;
        pname = packageJson.name;
        inherit (packageJson) version;
        pnpmDeps = pnpm.fetchDeps {
          inherit
            hash
            pname
            pnpmWorkspaces
            src
            version
            ;
        };
      in
        pkgs.buildNpmPackage (
          args
          // rec {
            inherit
              pname
              pnpmDeps
              pnpmWorkspaces
              src
              version
              ;
            npmConfigHook = pnpm.configHook;
            npmDeps = pnpmDeps;
          }
        );

      buildPnpmPackage = buildPnpmPackageFn {
        inherit pkgs;
        inherit (pkgs) lib;
      };
    in rec {
      packages = {
        codex-cli = buildPnpmPackage {
          inherit version hash;
          extraSrcs = [
            ./codex-cli
          ];
          packageJsonPath = ./codex-cli/package.json;
          src = ./.;
          buildInputs = [
            node
          ];
          buildPhase = ''
            runHook preBuild
            echo "pnpm --filter=codex-cli run build"
            pnpm --filter="@openai/codex" run build
            runHook postBuild
          '';
          installPhase = ''
            mkdir -p $out
            cp -r ./codex-cli/dist $out
          '';
          checkPhase = ''
            pnpm --filter=codex-cli run test
          '';
          npmInstallFlags = ["--frozen-lockfile"];
          pnpmWorkspaces = ["@openai/codex"];
          meta = with pkgs.lib; {
            description = "OpenAI Codex command‑line interface";
            license = licenses.asl20;
            homepage = "https://github.com/openai/codex";
          };
        };
      };
      defaultPackage = packages.codex-cli;
      devShell = pkgs.mkShell {
        name = "codex-cli-dev";
        buildInputs = [
          node
          pnpm
        ];
        shellHook = ''
          cd codex-cli
          pnpm install --frozen-lockfile
          echo "Entering development shell for codex-cli"
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
