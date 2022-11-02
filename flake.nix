{
  description = "Description for the project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = {
    self,
    flake-parts,
    ...
  }:
    flake-parts.lib.mkFlake {inherit self;} {
      systems = ["x86_64-linux" "aarch64-darwin"];
      perSystem = {
        self',
        pkgs,
        ...
      }: {
        packages.minidsp = pkgs.callPackage "${self}/default.nix" {
          inherit self;
        };
        packages.default = self'.packages.minidsp;
      };
    };
}
