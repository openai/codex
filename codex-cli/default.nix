{ pkgs, monorepo-deps ? [], ... }:

let
  codex-cli-src = pkgs.lib.cleanSource ./.;
  root-pnpm-lock = ../pnpm-lock.yaml;

  codex-cli-package = pkgs.stdenv.mkDerivation {
    pname = "codex-cli";
    version = "0.0.0-dev";
    src = codex-cli-src;

    nativeBuildInputs = [
      pkgs.nodejs
      pkgs.pnpm
    ];

    installPhase = ''
      mkdir -p $out/bin
      cp -r $src/* .
      cp ${root-pnpm-lock} pnpm-lock.yaml
      export PNPM_HOME=$(pwd)/.pnpm-store
      pnpm install --frozen-lockfile --ignore-scripts --store-dir $PNPM_HOME
      cp bin/codex.js $out/bin/
      chmod +x $out/bin/codex.js
      ln -s $out/bin/codex.js $out/bin/codex
    '';

    buildPhase = "true"; # No build step needed for a CLI
  };


in
rec {
  package = codex-cli-package;

  devShell = pkgs.mkShell {
    name = "codex-cli-dev";
    packages = monorepo-deps ++ [
      pkgs.nodejs
      pkgs.pnpm
    ];
    shellHook = ''
      echo "Entering development shell for codex-cli"
    '';
  };

  app = {
    type = "app";
    program = "${codex-cli-package}/bin/codex";
  };
}
