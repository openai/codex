{ pkgs, monorepo-deps ? [], ... }:

let
  codex-cli-package = pkgs.buildNpmPackage {
    pname = "codex-cli";
    version = "0.0.0-dev";
    src = ./.;
    npmDepsHash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="; # Placeholder, will be updated by nix build
    installPhase = ''
      pnpm install --frozen-lockfile --ignore-scripts
      mkdir -p $out/bin
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