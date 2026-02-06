{
  description = "Development Nix flake for OpenAI Codex CLI";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { nixpkgs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems f;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          codex-rs = pkgs.callPackage ./codex-rs { };
        in
        {
          codex-rs = codex-rs;
          default = codex-rs;
        }
      );
    };
}
