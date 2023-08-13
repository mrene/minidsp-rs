{
  description = "A control interface for some MiniDSP products";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-22.11";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        minidsp = pkgs.callPackage ./package.nix { };
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
          buildInputs = with pkgs;
            lib.optionals stdenv.isLinux [ libusb1 ] ++ 
            lib.optionals stdenv.isDarwin (with darwin.apple_sdk.frameworks; [ IOKit AppKit ]);

          nativeBuildInputs = with pkgs; [ pkg-config rust-bin.stable.latest.default pkgs.rust-bin.stable.latest.rust-analyzer ];
        };
      }) // {
        nixosModules.default = import ./module.nix self;
      };
}
