{ config, lib, pkgs, ... }:

let
  cfg = config.services.noctoggle;
in
{
  options.services.noctoggle = {
    enable = lib.mkEnableOption "noctoggle – a noctalia topbar toggler";

    systemdTarget = lib.mkOption {
      type = lib.types.str;
      default = "graphical-session.target";
      description = "The systemd target to bind the noctoggle service to.";
    };

    noctaliaPackage = lib.mkOption {
      type = lib.types.package;
      default = pkgs.noctalia;
      description = ''
        The noctalia package used to locate the binary for the default show/hide commands.
        Requires noctalia v5+; for v4, override showCommand and hideCommand with the old `ipc call` syntax.
      '';
    };

    showCommand = lib.mkOption {
      type = lib.types.str;
      description = "Command run when Super is first pressed.";
      default = "${lib.getExe cfg.noctaliaPackage} msg bar-show";
    };

    hideCommand = lib.mkOption {
      type = lib.types.str;
      description = "Command run when Super is fully released.";
      default = "${lib.getExe cfg.noctaliaPackage} msg bar-hide";
    };

    triggerKeys = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      description = ''
        List of keys that trigger the bar toggle (e.g. KEY_LEFTMETA).
        See https://docs.rs/evdev-keys/latest/evdev_keys/ for a list of supported keys.
      '';
      default = [ "KEY_LEFTMETA" "KEY_RIGHTMETA" ];
    };
  };

  config = lib.mkIf cfg.enable {

    systemd.user.services.noctoggle = {
      description = "noctoggle – noctalia topbar Super-key toggle";
      partOf = [ cfg.systemdTarget ];
      after = [ cfg.systemdTarget ];
      wantedBy = [ cfg.systemdTarget ];

      serviceConfig = {
        ExecStart = "${lib.getExe cfg.package}";
        Restart = "on-failure";
        RestartSec = 2;
        Environment = [
          "SHOW_CMD=${lib.escapeShellArg cfg.showCommand}"
          "HIDE_CMD=${lib.escapeShellArg cfg.hideCommand}"
          "TRIGGER_KEYS=${lib.concatStringsSep "," cfg.triggerKeys}"
        ];

        CapabilityBoundingSet = "";
        DeviceAllow = "char-input r";
        DevicePolicy = "strict";
        KeyringMode = "private";
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        NoNewPrivileges = true;
        PrivateNetwork = true;
        PrivateTmp = true;
        PrivateUsers = true;
        ProcSubset = "pid";
        ProtectClock = true;
        ProtectControlGroups = true;
        ProtectHome = "read-only";
        ProtectHostname = true;
        ProtectKernelLogs = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        ProtectNetwork = true;
        ProtectProc = "invisible";
        ProtectSystem = "strict";
        RestrictAddressFamilies = [ "AF_UNIX" ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = "@system-service";
        UMask = "0077";
      };
    };
  };
}
