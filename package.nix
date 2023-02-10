{ lib, rustPlatform, stdenv, libusb1 ? null, darwin ? null, pkg-config }:

rustPlatform.buildRustPackage {
  pname = "minidsp";
  version = "0.1.9";
  src = ./.;

  cargoBuildFlags = [ "-p minidsp -p minidsp-daemon" ];
  cargoLock.lockFile = ./Cargo.lock;

  doCheck = false;

  buildInputs = lib.optionals stdenv.isLinux [ libusb1 ] ++
    lib.optionals stdenv.isDarwin (with darwin.apple_sdk.frameworks; [ IOKit AppKit ]);

  nativeBuildInputs = lib.optionals stdenv.isLinux [ pkg-config ];

  meta = with lib; {
    description = "A control interface for some MiniDSP products";
    homepage = "https://github.com/mrene/minidsp-rs";
    license = licenses.asl20;
    platforms = platforms.linux ++ platforms.darwin;
  };
}
