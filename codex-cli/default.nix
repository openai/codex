{ pkgs, monorep-deps ? [], ... }:
let
  node = pkgs.nodejs_22;
  pnpm = pkgs.pnpm_10;                 
in
rec {
  package = pkgs.stdenv.mkDerivation (final: {
    pname   = "codex-cli";
    version = "0.1.0";
    src     = ./..;                      

    pnpmDeps = pnpm.fetchDeps {
      inherit (final) pname version src;
      hash = "sha256-SyKP++eeOyoVBFscYi+Q7IxCphcEeYgpuAj70+aCdNA=";
    };

    nativeBuildInputs = [
      node 
      pnpm 
      pnpm.configHook         
      pkgs.jq                          
      pkgs.makeWrapper
    ];

    pnpmInstallFlags = [ "--frozen-lockfile" ];

    buildPhase = ''
      runHook preBuild
      pnpm install --offline --frozen-lockfile
      pnpm --filter ./codex-cli... run build
      runHook postBuild
    '';

    installPhase = ''
      runHook preInstall

      pkgRoot=$out/lib/node_modules/${final.pname}
      mkdir -p $pkgRoot
      cp -R codex-cli/* $pkgRoot/

      path=$(jq -r '.bin.codex' codex-cli/package.json)
      mkdir -p $out/bin
      makeWrapper ${node}/bin/node $out/bin/codex --add-flags "$pkgRoot/$path"

      runHook postInstall
    '';

    meta = with pkgs.lib; {
      description = "OpenAI Codex command‑line interface";
      license     = licenses.asl20;
      homepage    = "https://github.com/openai/codex";
    };
  });
  devShell = pkgs.mkShell {
    name        = "codex-cli-dev";
    buildInputs = monorep-deps ++ [ node pnpm ];
    shellHook = ''
      echo >&2 "Entering development shell for codex-cli"
      pnpm install && echo "pnpm install succeded" || echo "npm install failed"
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

