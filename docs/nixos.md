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
            printerUrl = "http://printer.local/";
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

By default the service listens on `127.0.0.1:3000` and leaves the firewall closed,
which also suits a local reverse proxy. IPv6 addresses should be unbracketed,
for example `listenAddress = "::1"`. The HTTP endpoint has no authentication;
expose it only to trusted clients or through an authenticated reverse proxy.
Run only one proxy per printer because the printer preview object is shared.
Use a dedicated printer with trusted default syntax, bitmap clearing, and font
mappings; see the [admission policy and printer state prerequisite](proxy-validation.md).

The systemd service runs as a dynamic user, serves the packaged browser assets,
and stores its SQLite database at `/var/lib/zpl-proxy-api/db.sqlite`. Systemd
manages the directory ownership and keeps it across restarts. Diesel migrations
run before every start, including upgrades; migration failure prevents startup.
Back up the database before upgrading. To take a simple offline backup, stop
`zpl-proxy-api.service` and copy its state directory (dereferencing the systemd
symlink if present). NixOS rollback does not roll back the database schema.

Additional options:

- `cacheNamespace`: defaults to `"default"`; change after firmware, font, media,
  or rendering configuration changes to invalidate cached results.
- `printerHeaders`: a list of `"Name: value"` headers. These are visible in the
  Nix store and process arguments, so use only nonsecret values.
- `environment`: additional variables such as `RUST_LOG` and `OTEL_*`; see
  [telemetry configuration](telemetry.md).
- `environmentFile`: optional runtime path such as
  `"/run/secrets/zpl-proxy-api.env"` for systemd environment settings, including
  OTLP credentials. Keep it outside the Nix store. Do not override `DATABASE_URL`.
- `package`: override the proxy package. It must include the binary and
  `share/zpl-proxy-api/{assets,migrations}`.

Inspect service status and logs with `systemctl status zpl-proxy-api` and
`journalctl -u zpl-proxy-api`. The UI is at `/` and the rendering endpoint is
`POST /api/zpl-zd621`; see [cache behavior](proxy-cache.md).

Build the package on Linux with `nix build .#zpl-proxy-api`. Run the NixOS
integration test with `nix build .#checks.x86_64-linux.zpl-proxy-api` on a
Linux builder with KVM support.
