{ pkgs ? import <nixpkgs> { } }:
pkgs.rustPlatform.buildRustPackage rec {
  pname = "flow";
  version = "0.1";
  cargoLock.lockFile = ./Cargo.lock;
  src = pkgs.lib.cleanSource ./.;
}
