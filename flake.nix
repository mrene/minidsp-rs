{
  description = "A control interface for some MiniDSP products";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        minidsp = pkgs.rustPlatform.buildRustPackage {
          pname = "minidsp";
          version = "0.1.9";
          src = ./.;
          cargoBuildFlags = [ "-p minidsp -p minidsp-daemon --release" ];
          cargoLock.lockFile = ./Cargo.lock;
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

        apps.default = flake-utils.lib.mkApp {
          drv = minidsp;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = minidsp.buildInputs;
          nativeBuildInputs = with pkgs; [
            cargo
            rustc
          ];
        };
      });
}