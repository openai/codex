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
      ppnpm = pkgs.pnpm_10;
      version = "0.1.0";
      hash = "sha256-pPwHjtqqaG+Zqmq6x5o+WCT1H9XuXAqFNKMzevp7wTc=";

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
        pnpm ? ppnpm,
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
            echo "pnpm --filter=@openai/codex run build"
            pnpm --filter="@openai/codex" run build
            runHook postBuild
          '';
          installPhase = ''
            runHook preInstall

            mkdir -p $out/lib/node_modules/@openai/codex
            cp -r ./codex-cli/dist $out/lib/node_modules/@openai/codex/dist
            cp ./codex-cli/package.json $out/lib/node_modules/@openai/codex/

            mkdir -p $out/bin

            cp $out/lib/node_modules/@openai/codex/dist/cli.js $out/bin/codex

            # Make the script executable
            chmod +x $out/bin/codex

            runHook postInstall
          '';
          checkPhase = ''
            pnpm --filter="@openai/codex" run test
          '';
          npmInstallFlags = ["--frozen-lockfile"];
          pnpmWorkspaces = ["@openai/codex"];
          meta = with pkgs.lib; {
            description = "OpenAI Codex command‑line interface";
            license = licenses.asl20;
            homepage = "https://github.com/openai/codex";
          };
        };
        update-nix-hash =
          pkgs.writers.writePython3Bin "update-nix-hash" {
          } ''
            import os
            import subprocess
            import sys

            # Get the repository root
            repo_root = (
                subprocess.check_output([
                    "git",
                    "rev-parse",
                    "--show-toplevel"
                ])
                .decode()
                .strip()  #
            )
            os.chdir(repo_root)

            # Run nix build command and capture output
            try:
                build_output = subprocess.check_output(  #
                    [
                       "nix",
                       "build",
                       ".#codex-cli",
                       "--show-trace"
                    ],  #
                    stderr=subprocess.STDOUT,  #
                    universal_newlines=True,  #
                )  #
            except subprocess.CalledProcessError as e:
                build_output = e.output

            # Extract the "got" hash by going line by line
            new_hash = None
            for line in build_output.splitlines():
                if "got:" in line:
                    parts = line.split()
                    if len(parts) >= 2:
                        new_hash = parts[1]
                        break

            # Check if we found a hash
            if new_hash:
                print(f"Extracted got hash: {new_hash}")
            else:
                print("Could not extract got hash from build output.")
                print("Did pnpm-lock.yaml change?")
                print("Full build output:")
                print(build_output)
                sys.exit(1)

            # Update the hash in flake.nix line by line
            flake_path = os.path.join(repo_root, "flake.nix")
            updated_lines = []
            found_hash_line = False
            with open(flake_path, "r") as file:
                for line in file:
                    if 'hash = "' in line and not found_hash_line:
                        found_hash_line = True
                        # Split the line at the hash value
                        parts = line.split('hash = "')
                        prefix = parts[0] + 'hash = "'

                        # Split the remaining part at the closing quote
                        if '"' in parts[1]:
                            rest_parts = parts[1].split('"', 1)
                            suffix = '"' + rest_parts[1]

                            # Create the new line with the updated hash
                            line = prefix + new_hash + suffix
                    updated_lines.append(line)

            # Write the updated content back to the file
            with open(flake_path, "w") as file:
                file.writelines(updated_lines)

            # Check if the hash was actually changed
            try:
                git_diff = subprocess.check_output(
                    ["git", "diff", flake_path], universal_newlines=True
                )
                if not git_diff:
                    print("Hash is already up to date. No changes needed.")
                    sys.exit(0)
            except subprocess.CalledProcessError:
                # Handle potential git diff error
                pass

            # If running in GitHub Actions environment, configure git user
            if os.environ.get("GITHUB_ACTIONS"):
                _ = subprocess.run(["git", "config", "user.name", "GitHub Actions"])
                _ = subprocess.run(["git", "config", "user.email", "actions@github.com"])

                # Commit and push the change
                _ = subprocess.run(["git", "add", flake_path])
                commit_message = """chore: Update pnpm deps hash in flake.nix
            This automated commit updates the pnpm deps hash hash in flake.nix to match
            the latest hash of the workspace's pnpm-lock.yaml file. This ensures that
            Nix's store is consistent with the latest dependencies."""
                _ = subprocess.run(["git", "commit", "-m", commit_message])
                _ = subprocess.run(["git", "push"])
            else:
                print("Hash has been updated in flake.nix.")
                print("You may want to commit this change with:")
                print(f"  git add {flake_path}")
                print('  git commit -m "chore: Update npmDepsHash in flake.nix"')

            print("Successfully updated npmDepsHash in flake.nix")
          '';
      };
      defaultPackage = packages.codex-cli;
      default = packages.codex-cli;
      devShells.default = pkgs.mkShell {
        name = "codex-cli-dev";
        buildInputs = [
          node
          ppnpm
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
