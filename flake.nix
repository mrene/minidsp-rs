{
  description = "A control interface for some MiniDSP products";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, flake-utils, flake-compat, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        minidsp = pkgs.rustPlatform.buildRustPackage {
          pname = "minidsp";
          version = "0.1.9";
          src = ./.;
          cargoBuildFlags = [ "-p minidsp -p minidsp-daemon" ];
          cargoLock.lockFile = ./Cargo.lock;
          doCheck = false;
          buildInputs = with pkgs;
            lib.optionals stdenv.isLinux [ libusb1 ] ++ 
            lib.optionals stdenv.isDarwin (with darwin.apple_sdk.frameworks; [ IOKit AppKit ]);

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
        };
      in
      { 
        packages.default = minidsp;

        apps = { 
          default = flake-utils.lib.mkApp {
            drv = minidsp;
          };
          minidspd = flake-utils.lib.mkApp {
            drv = minidsp;
            exePath = "/bin/minidspd";
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = minidsp.buildInputs;
          # buildRustPackage defines the baseline native build inputs
          nativeBuildInputs = minidsp.nativeBuildInputs;
        };
      });
}
