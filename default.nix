{
  fetchFromGitHub,
  hidapi,
  libusb1,
  pkg-config,
  rustPlatform,
  lib,
  self,
}: let
  # Filter out VCS files and files unrelated to the Rust ragenix package
  filterRustSource = src:
    with lib;
      cleanSourceWith {
        filter = cleanSourceFilter;
        src = cleanSourceWith {
          inherit src;
          filter = name: type: let
            pathWithoutPrefix = removePrefix (toString src) name;
          in
            ! (
              hasPrefix "/.github" pathWithoutPrefix
              || pathWithoutPrefix == "/.gitignore"
              || pathWithoutPrefix == "/LICENSE"
              || pathWithoutPrefix == "/README.md"
              || pathWithoutPrefix == "/README-API.md"
              || pathWithoutPrefix == "/flake.lock"
              || pathWithoutPrefix == "/flake.nix"
            );
        };
      };
  rustSource = filterRustSource self;
in
  rustPlatform.buildRustPackage {
    pname = "minidsp";
    version = "0.1.9";

    src = rustSource;

    cargoLock.lockFile = ./Cargo.lock;

    strictDeps = true;
    buildInputs = [
      libusb1
      hidapi
    ];
    nativeBuildInputs = [
      pkg-config
    ];
  }
