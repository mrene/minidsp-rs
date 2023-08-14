self: { config, lib, pkgs, ... }:

with lib;
let
  cfg = config.services.minidsp;
  tomlFormat = pkgs.formats.toml {};
in
{
  options = {
    services.minidsp = {
      enable = mkOption {
        type = types.bool;
        default = false;
        description = ''
        Whether to enable the minidsp service
        '';
      };

      package = mkOption {
        type = types.package;
        default = self.packages.${pkgs.system}.default;
        description = ''
        minidsp package to use
        '';
      };

      config = mkOption {
        type = tomlFormat.type;
        default = {
          http_server = {
            bind_address = "127.0.0.1:5380";
          };

          tcp_server = [{
            bind_address = "127.0.0.1:5333";
          }];
        };
        description = ''
        Configuration file
        For available options see <https://minidsp-rs.pages.dev/daemon/>
        '';
      };
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    users = {
      users.minidsp = {
        description = "minidsp daemon user";
        group = "minidsp";
        isSystemUser = true;
      };
      groups.minidsp = {};
    };

    systemd.services.minidsp = {
      after = ["network.target"];
      description = "minidsp daemon";
      wantedBy = ["multi-user.target"];

      serviceConfig = {
        User = "minidsp";
        ExecStart = "${cfg.package}/bin/minidspd --config /etc/minidsp/config.toml";
      };
    };

    environment.etc."minidsp/config.toml" = {
      source = tomlFormat.generate "config.toml" cfg.config;
    };

    services.udev.extraRules = ''
      ATTR{idVendor}=="2752", MODE="660", GROUP="minidsp"
      ATTR{idVendor}=="04d8", ATTRS{idProduct}=="003f", MODE="660", GROUP="minidsp"
    '';
  };
}
