{
  description = "convert-case flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      name = "convert-case";
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      lib = pkgs.lib;
    in
    {
      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        pname = name;
        version = (lib.importTOML ./Cargo.toml).package.version;
        src = ./.;
        cargoLock = {
          lockFile = ./Cargo.lock;
        };
      };

      devShells.${system}.default = pkgs.mkShell {
        name = "convert-case";
        buildInputs = with pkgs; [
          just
          watchexec
          rustup
          #cargo-tarpaulin
        ];
      };
    };
}
