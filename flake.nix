{
  description = "Development Nix flake for OpenAI Codex CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, ... }: 
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs { inherit system; };
      node = pkgs.nodejs_22;
    in rec {
      packages = {
        codex-cli = pkgs.buildNpmPackage rec {
          pname       = "codex-cli";
          version     = "0.1.0";
          src         = self + "/codex-cli";
          npmDepsHash = "sha256-riVXC7T9zgUBUazH5Wq7+MjU1FepLkp9kHLSq+ZVqbs=";
          nodejs      = node;
          npmInstallFlags = [ "--frozen-lockfile" ];
          meta = with pkgs.lib; {
            description = "OpenAI Codex command‑line interface";
            license     = licenses.asl20;
            homepage    = "https://github.com/openai/codex";
          };
        };
      };
      defaultPackage = packages.codex-cli;
      devShell = pkgs.mkShell {
        name        = "codex-cli-dev";
        buildInputs = [
          node
        ];
        shellHook = ''
          echo "Entering development shell for codex-cli"
          cd codex-cli
          npm ci
          npm run build
          export PATH=$PWD/node_modules/.bin:$PATH
          alias codex="node $PWD/dist/cli.js"
        '';
      };
      apps = {
        codex = {
          type    = "app";
          program = "${packages.codex-cli}/bin/codex";
        };
      };
    });
}
