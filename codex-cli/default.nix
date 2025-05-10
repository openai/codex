{ pkgs, monorep-deps ? [], ... }:
let
  node = pkgs.nodejs_22;
in
rec {
  package = pkgs.buildNpmPackage {
    pname       = "codex-cli";
    version     = "0.1.0";
    src         = ./.;
    npmDepsHash = "sha256-3tAalmh50I0fhhd7XreM+jvl0n4zcRhqygFNB1Olst8";
    nodejs      = node;
    npmInstallFlags = [ "--frozen-lockfile" ];
    meta = with pkgs.lib; {
      description = "OpenAI Codex command‑line interface";
      license     = licenses.asl20;
      homepage    = "https://github.com/openai/codex";
    };
  };
  devShell = pkgs.mkShell {
    name        = "codex-cli-dev";
    buildInputs = monorep-deps ++ [
      node
      pkgs.pnpm
    ];
    shellHook = ''
      echo "Entering development shell for codex-cli"
      # cd codex-cli
      if [ -f package-lock.json ]; then
        pnpm ci || echo "npm ci failed"
      else
        pnpm install || echo "npm install failed"
      fi
      npm run build || echo "npm build failed"
      export PATH=$PWD/node_modules/.bin:$PATH
      alias codex="node $PWD/dist/cli.js"
    '';
  };
  app = {
    type    = "app";
    program = "${package}/bin/codex";
  };
}

