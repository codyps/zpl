{ config, lib, pkgs, utils, ... }:

let
  cfg = config.services.zpl-proxy-api;
  bindAddress = if lib.hasInfix ":" cfg.listenAddress then "[${cfg.listenAddress}]" else cfg.listenAddress;
  printerConfig = pkgs.writeText "zpl-printers.json" (builtins.toJSON cfg.printers);
  arguments = [ "--socket-activation" "--cache-namespace" cfg.cacheNamespace "--printers" printerConfig ];

in
{
  options.services.zpl-proxy-api = {
    enable = lib.mkEnableOption "the printer-backed ZPL rendering proxy";
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.callPackage ./package.nix { };
      defaultText = lib.literalExpression "pkgs.callPackage ./package.nix { }";
      description = "Proxy package, including assets and migrations under share/zpl-proxy-api.";
    };
    printers = lib.mkOption {
      default = [ ];
      description = "Named, exclusively owned printers. Public requests select name via /api/printers/{name}/preview. Values are public in the Nix store.";
      type = lib.types.listOf (lib.types.submodule {
        options = {
          name = lib.mkOption { type = lib.types.strMatching "[A-Za-z0-9_.-]{1,80}"; description = "Public printer name, typically model or model-firmware."; };
          url = lib.mkOption { type = lib.types.str; description = "Printer HTTP(S) origin."; };
          control_address = lib.mkOption { type = lib.types.str; description = "Trusted SGD host:port, usually printer:9100."; };
          width = lib.mkOption { type = lib.types.ints.between 8 32000; description = "Default native canvas width in dots."; };
          height = lib.mkOption { type = lib.types.ints.between 8 32000; description = "Default canvas height in dots."; };
          headers = lib.mkOption { type = lib.types.listOf lib.types.str; default = [ ]; description = "HTTP headers; do not put secrets in the Nix store."; };
          serial = lib.mkOption { type = lib.types.nullOr lib.types.str; default = null; description = "Optional expected serial number; mismatches fail closed."; };
        };
      });
    };
    listenAddress = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      example = "0.0.0.0";
      description = "TCP address when unixSocket is null; use an unbracketed address for IPv6.";
    };
    port = lib.mkOption {
      type = lib.types.port;
      default = 3000;
      description = "TCP port when unixSocket is null.";
    };
    unixSocket = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "/run/zpl-proxy-api.sock";
      description = "Absolute Unix socket path instead of TCP. Systemd owns the socket.";
    };
    unixSocketMode = lib.mkOption {
      type = lib.types.strMatching "[0-7]{3,4}";
      default = "0660";
      description = "Filesystem permissions for the Unix socket.";
    };
    unixSocketGroup = lib.mkOption {
      type = lib.types.str;
      default = "root";
      example = "nginx";
      description = "Existing group allowed to connect to the Unix socket.";
    };
    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether to open the TCP port in the firewall; has no effect with unixSocket.";
    };
    cacheNamespace = lib.mkOption {
      type = lib.types.str;
      default = "default";
      description = "Change after printer firmware, fonts, media, or rendering configuration changes.";
    };
    environment = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
      example = { RUST_LOG = "info"; OTEL_TRACES_EXPORTER = "none"; };
      description = "Additional environment variables, for example RUST_LOG and OTEL_* telemetry settings.";
    };
    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "/run/secrets/zpl-proxy-api.env";
      description = "Runtime systemd environment file, for example for OTLP credentials. Do not override DATABASE_URL.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.printers != [ ];
        message = "Configure at least one services.zpl-proxy-api.printers entry.";
      }

      {
        assertion = cfg.unixSocket == null || lib.hasPrefix "/" cfg.unixSocket;
        message = "services.zpl-proxy-api.unixSocket must be an absolute filesystem path.";
      }
      {
        assertion = !(cfg.environment ? DATABASE_URL);
        message = "services.zpl-proxy-api manages DATABASE_URL in its persistent state directory.";
      }
    ];

    networking.firewall.allowedTCPPorts = lib.mkIf (cfg.openFirewall && cfg.unixSocket == null) [ cfg.port ];
    # One shared listener (Accept=no); the proxy serializes printer access.
    # https://www.freedesktop.org/software/systemd/man/latest/systemd.socket.html
    systemd.sockets.zpl-proxy-api = {
      description = "ZPL printer rendering proxy socket";
      wantedBy = [ "sockets.target" ];
      listenStreams = [
        (if cfg.unixSocket != null then cfg.unixSocket else "${bindAddress}:${toString cfg.port}")
      ];
      socketConfig = {
        Accept = false;
      } // lib.optionalAttrs (cfg.unixSocket != null) {
        SocketMode = cfg.unixSocketMode;
        SocketGroup = cfg.unixSocketGroup;
        DirectoryMode = "0755";
        RemoveOnStop = true;
      };
    };
    systemd.services.zpl-proxy-api = {
      description = "ZPL printer rendering proxy";
      requires = [ "zpl-proxy-api.socket" ];
      wants = [ "network-online.target" ];
      after = [ "network-online.target" "zpl-proxy-api.socket" ];
      # Mount only the ExecStart/ExecStartPre runtime closures, not the whole
      # store or unit closure (which can include unrelated environment paths).
      # https://github.com/NixOS/nixpkgs/blob/master/nixos/modules/security/systemd-confinement.nix
      confinement = {
        enable = true;
        mode = "chroot-only";
        binSh = null;
      };
      environment = cfg.environment // {
        DATABASE_URL = "/var/lib/zpl-proxy-api/db.sqlite";
      };
      # Diesel records applied migrations, so this also handles subsequent upgrades.
      # https://diesel.rs/guides/getting-started.html
      preStart = ''
        ${lib.getExe pkgs.diesel-cli} migration run --migration-dir ${cfg.package}/share/zpl-proxy-api/migrations
      '';
      serviceConfig = {
        ExecStart = utils.escapeSystemdExecArgs ([ (lib.getExe cfg.package) ] ++ arguments);
        WorkingDirectory = "${cfg.package}/share/zpl-proxy-api";
        Restart = "on-failure";
        RestartSec = 5;
        # StateDirectory is writable and persists across DynamicUser lifetimes.
        # https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html#StateDirectory=
        DynamicUser = true;
        StateDirectory = "zpl-proxy-api";
        StateDirectoryMode = "0700";
        UMask = "0077";
        EnvironmentFile = lib.mkIf (cfg.environmentFile != null) cfg.environmentFile;
        # Systemd binds even privileged ports; the worker needs no capabilities.
        CapabilityBoundingSet = "";
        NoNewPrivileges = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        # RootDirectory confinement hides host data; StateDirectory is mounted
        # automatically. EnvironmentFile is read by the host service manager.
        # systemd.exec(5), BindReadOnlyPaths=, StateDirectory=, EnvironmentFile=:
        # https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html
        BindReadOnlyPaths = [
          "/etc/hosts"
          "/etc/resolv.conf"
          "/etc/nsswitch.conf"
          "/etc/ssl/certs/ca-certificates.crt"
          printerConfig
        ];
        MountAPIVFS = true;
        ProtectProc = "invisible";
        ProcSubset = "pid";
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictSUIDSGID = true;
        RestrictAddressFamilies = [ "AF_UNIX" "AF_INET" "AF_INET6" ];
      };
    };
  };
}
