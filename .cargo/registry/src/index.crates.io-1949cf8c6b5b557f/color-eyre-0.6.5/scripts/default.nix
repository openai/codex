{ pkgs ? import <nixpkgs> { } }:
let
  inherit (pkgs) stdenv lib python38;

  py = python38.withPackages (pypkgs: with pypkgs; [ beautifulsoup4 ]);

in stdenv.mkDerivation {
  pname = "color-eyre-scripts";
  version = "0.0.0";

  src = ./.;
  buildInputs = [ py ];
}
