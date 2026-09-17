{ config, lib, pkgs, utils, ... }:

let
  cfg = config.services.zpl-proxy-api;
  bindAddress = if lib.hasInfix ":" cfg.listenAddress then "[${cfg.listenAddress}]" else cfg.listenAddress;
  arguments = [
    "--zd621-url"
    cfg.printerUrl
    "--bind-addr"
    "${bindAddress}:${toString cfg.port}"
    "--cache-namespace"
    cfg.cacheNamespace
  ] ++ lib.concatMap (header: [ "--zd621-header" header ]) cfg.printerHeaders;
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
    printerUrl = lib.mkOption {
      type = lib.types.str;
      example = "http://printer.local/";
      description = "Base HTTP URL of the Zebra printer. Run only one proxy per printer.";
    };
    printerHeaders = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "X-Example: value" ];
      description = "Extra printer headers in 'Name: value' format. Values are public in the Nix store; do not put secrets here.";
    };
    listenAddress = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      example = "0.0.0.0";
      description = "IP address to listen on; use an unbracketed address for IPv6.";
    };
    port = lib.mkOption {
      type = lib.types.port;
      default = 3000;
      description = "HTTP listening port.";
    };
    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether to open the HTTP port in the firewall.";
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
        assertion = lib.hasPrefix "http://" cfg.printerUrl || lib.hasPrefix "https://" cfg.printerUrl;
        message = "services.zpl-proxy-api.printerUrl must be an HTTP(S) URL.";
      }
      {
        assertion = lib.all (header: lib.hasInfix ": " header) cfg.printerHeaders;
        message = "services.zpl-proxy-api.printerHeaders must use 'Name: value' format.";
      }
      {
        assertion = !(cfg.environment ? DATABASE_URL);
        message = "services.zpl-proxy-api manages DATABASE_URL in its persistent state directory.";
      }
    ];

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.port ];
    systemd.services.zpl-proxy-api = {
      description = "ZPL printer rendering proxy";
      wantedBy = [ "multi-user.target" ];
      wants = [ "network-online.target" ];
      after = [ "network-online.target" ];
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
        AmbientCapabilities = lib.optional (cfg.port < 1024) "CAP_NET_BIND_SERVICE";
        CapabilityBoundingSet = lib.optional (cfg.port < 1024) "CAP_NET_BIND_SERVICE";
        NoNewPrivileges = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictSUIDSGID = true;
        RestrictAddressFamilies = [ "AF_UNIX" "AF_INET" "AF_INET6" ];
      };
    };
  };
}
