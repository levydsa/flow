{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils = {
      url = "github:numtide/flake-utils";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, crane }:
    flake-utils.lib.eachDefaultSystem (system:
    let 
      craneLib = crane.mkLib pkgs;

      pkgs = import nixpkgs {
        inherit system;
      };
    in
    rec {
      formatter = pkgs.nixpkgs-fmt;
      packages.flow = craneLib.buildPackage {
        src = ./.;
        strictDeps = true;
      };
      packages.default = packages.flow;
    });
}
