{
  description = "A control interface for some MiniDSP products";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.05";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
    nix-filter.url = "github:numtide/nix-filter";
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, nix-filter, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        minidsp = pkgs.callPackage ./package.nix { };
      in
      {
        packages.default = minidsp.overrideAttrs(prev: {
          src = nix-filter.lib {
            root = ./.;
            include = [
              (nix-filter.lib.inDirectory "minidsp")
              (nix-filter.lib.inDirectory "protocol")
              (nix-filter.lib.inDirectory "daemon")
              (nix-filter.lib.inDirectory "devtools")
              (nix-filter.lib.inDirectory ".cargo")
              "Cargo.toml"
              "Cargo.lock"
            ];
          };
        });

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
