# Hosting the printer proxy on NixOS

The flake exports `nixosModules.zpl-proxy-api` (also `nixosModules.default`)
and `packages.<linux-system>.zpl-proxy-api`. Add this repository as a flake input
and import the module in your host configuration:

```nix
{
  inputs.zpl.url = "github:codyps/zpl";

  outputs = { nixpkgs, zpl, ... }: {
    nixosConfigurations.print-server = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ./configuration.nix
        zpl.nixosModules.zpl-proxy-api
        {
          services.zpl-proxy-api = {
            enable = true;
            printers.ZD621 = {
              url = "http://printer.local/";
              control_address = "printer.local:9100";
              width = 832;
              height = 1218;
            };
            # For direct access from the LAN:
            listenAddress = "0.0.0.0";
            port = 3000;
            openFirewall = true;
          };
        }
      ];
    };
  };
}
```

The module uses socket activation: `zpl-proxy-api.socket` starts at boot, and the
first connection starts `zpl-proxy-api.service`. Systemd keeps the listener open
across worker restarts. The worker stays running until stopped; there is no idle
shutdown timer. Migrations run before the worker begins accepting connections.

By default the socket listens on `127.0.0.1:3000` and leaves the firewall closed,
which also suits a local reverse proxy. IPv6 addresses should be unbracketed,
for example `listenAddress = "::1"`. The HTTP endpoint has no authentication;
expose it only to trusted clients or through an authenticated reverse proxy.
One proxy can own multiple named printers, with independent queues. Run only one
proxy instance per physical printer because its preview object is shared.
See [configuration, identity recording and automatic recovery](proxy-cache.md).
Use a dedicated printer with trusted default syntax, bitmap clearing, and font
mappings; see the [admission policy and printer state prerequisite](proxy-validation.md).

## Runtime secrets with sops-nix

Use `printers` only for nonsecret settings: those values enter the Nix store.
For credentials or private printer addresses/serials, set `printersFile` instead.
It is an absolute runtime **string** path to the complete JSON printer array;
configure exactly one of `printers` and `printersFile`. Do not use
`builtins.readFile` on a decrypted secret or put plaintext secrets in `pkgs.writeText`.

Following nixpkgs' [Inadyn config-file option](https://github.com/NixOS/nixpkgs/blob/master/nixos/modules/services/networking/inadyn.nix)
and [Stalwart credentials](https://github.com/NixOS/nixpkgs/blob/master/nixos/modules/services/mail/stalwart.nix),
the module uses systemd `LoadCredential`. Systemd reads the source as root and
makes a private copy available to the dynamic service user inside its confined
filesystem. The original secret path is not mounted into the worker. Both public
generated configuration and runtime secret files use the same credential path.
No secret values enter command arguments, and configuration parsing errors omit
input values.

For example, with the sops-nix module already imported and its decryption key
configured, store the complete printer JSON object keyed by printer name as a
YAML string secret named `zpl-printers` in your encrypted `secrets.yaml`:

```nix
{ config, ... }: {
  sops.secrets.zpl-printers = {
    sopsFile = ./secrets.yaml;
    owner = "root";
    mode = "0400";
    restartUnits = [ "zpl-proxy-api.service" ];
  };
  services.zpl-proxy-api = {
    enable = true;
    printersFile = config.sops.secrets.zpl-printers.path;
  };
}
```

The decrypted value is the JSON object documented in
[proxy configuration](proxy-cache.md), including any `headers` such as `Authorization: Bearer ...`. The entire object can
be encrypted, so nonsecret fields and secret fields can coexist without custom
substitution rules. Alternatively, point `printersFile` at a
`sops.templates.<name>.path`; ensure the rendered result is valid JSON, including
proper escaping of secret strings. No direct sops-nix dependency is required by
this module, and other runtime secret-file providers work as well.

Secrets are loaded on service startup, not watched. Use sops-nix
[`restartUnits`](https://github.com/Mic92/sops-nix#restartingreloading-systemd-units-on-secret-change)
for rotation; manually replacing a file also requires a service restart. A missing
secret or invalid JSON fails startup. The module rejects `printersFile` paths in
the Nix store. `environmentFile` can likewise point at a sops-managed dotenv file
for telemetry credentials; it does not substitute variables into printer JSON.

For a local reverse proxy, select a Unix socket instead of TCP:

```nix
services.zpl-proxy-api = {
  enable = true;
  printers.ZD621 = {
    url = "http://printer.local/";
    control_address = "printer.local:9100";
    width = 832;
    height = 1218;
  };
  unixSocket = "/run/zpl-proxy-api.sock";
  unixSocketGroup = "nginx"; # An existing group used by the reverse proxy.
  unixSocketMode = "0660";
};
```

`unixSocket` defaults to `null` (TCP). When set, `listenAddress`, `port`, and
`openFirewall` have no effect. Systemd creates parent directories as needed and
owns the socket as root with the configured group (`root` by default). It removes
the socket when the socket unit stops. The dynamic worker receives the open file
descriptor and needs neither filesystem socket ownership nor bind capabilities.

Test the Unix endpoint with:

```sh
curl --unix-socket /run/zpl-proxy-api.sock http://localhost/
```

The systemd service runs as a dynamic user, serves the packaged browser assets,
and stores its SQLite database at `/var/lib/zpl-proxy-api/db.sqlite`. Systemd
manages the directory ownership and keeps it across restarts. The proxy executable applies its embedded Diesel migrations
on every startup, including upgrades; migration failure prevents serving requests.
No separate Diesel CLI step is required.
Back up the database before upgrading. To take a simple offline backup, stop
both units with `systemctl stop zpl-proxy-api.socket zpl-proxy-api.service`, then
copy its state directory (dereferencing the systemd symlink if present). NixOS rollback does not roll back the database schema.

The worker, including startup migrations, runs inside a minimal filesystem namespace. Only their
Nix runtime closures, `/etc/hosts`, `/etc/resolv.conf`, `/etc/nsswitch.conf`, and
the system CA bundle are exposed, alongside the service credential directory, private temporary/device filesystems,
restricted proc/sys interfaces, and the writable state directory. Unrelated host
files and Nix store paths are hidden. Systemd reads `environmentFile` outside the
namespace; the worker receives its variables without access to the source file.
Custom file-based telemetry certificates or other runtime inputs require explicit
`systemd.services.zpl-proxy-api.serviceConfig.BindReadOnlyPaths` entries.

Additional options:

- `cacheNamespace`: defaults to `"default"`; change after firmware, font, media,
  or rendering configuration changes to invalidate cached results.
- `printers.<name>.headers`: a list of `"Name: value"` headers. These are visible in the
  Nix store, so use only nonsecret values.
- `environment`: additional variables such as `RUST_LOG` and `OTEL_*`; see
  [telemetry configuration](telemetry.md).
- `environmentFile`: optional runtime path such as
  `"/run/secrets/zpl-proxy-api.env"` for systemd environment settings, including
  OTLP credentials. Keep it outside the Nix store. Do not override `DATABASE_URL`.
- `package`: override the proxy package. It must include the binary and
  `share/zpl-proxy-api/assets` and embed its migrations.

Inspect status with `systemctl status zpl-proxy-api.socket zpl-proxy-api.service`
and logs with `journalctl -u zpl-proxy-api`. Stopping only the service leaves
socket activation enabled; a new connection starts it again. Start the socket
unit after maintenance with `systemctl start zpl-proxy-api.socket`. The UI is at
`/` and the rendering endpoint is
`POST /api/printers/{name}/preview`; see [cache behavior](proxy-cache.md).

Build the package on Linux with `nix build .#zpl-proxy-api`. Run the NixOS
integration test with `nix build .#checks.x86_64-linux.zpl-proxy-api` on a
Linux builder with KVM support.

## Standalone listener modes

Outside the NixOS module, select exactly one listener option while keeping the
database environment and parent directory and `zpl-proxy-api/` working directory:

```sh
cargo run -p zpl-proxy-api -- --printers printers.json --bind-addr 127.0.0.1:3000
cargo run -p zpl-proxy-api -- --printers printers.json --unix-socket /tmp/zpl-proxy.sock
```

A standalone Unix listener requires an existing parent directory. Permissions
follow the process umask. An existing file/socket is never automatically removed
at startup. Normal shutdown removes the socket created by that process; after a
crash, remove a stale socket explicitly before restarting.

`--socket-activation` accepts exactly one TCP or Unix stream listener through
systemd's native `LISTEN_PID`/`LISTEN_FDS` protocol (descriptor 3, `Accept=no`). It
fails if no descriptor is provided, the PID differs, or multiple descriptors are
passed. It never falls back to binding an address. With a custom systemd unit,
use one `ListenStream=` entry and start the executable directly so its PID matches
`LISTEN_PID`. Socket activation and Unix listeners are available on Unix platforms.

`cargo test -p zpl-proxy-api --test listeners` exercises real child-process TCP
and Unix descriptor inheritance, standalone Unix HTTP, graceful shutdown, invalid
activation settings, and CLI conflicts without requiring a running systemd.
The NixOS VM test covers lazy activation, service reactivation, migrations/cache,
and Unix socket permissions.
