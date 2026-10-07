{ pkgs }:

let
  common = {
    imports = [ ./module.nix ];
    services.zpl-proxy-api = {
      enable = true;
      printers.ZD621 = {
        url = "http://printer.test:9100/";
        control_address = "printer.test:9101";
        width = 64;
        height = 32;
      };
      environment.OTEL_TRACES_EXPORTER = "none";
      environmentFile = "/run/proxy-test.env";
    };
    environment.systemPackages = [ pkgs.curl pkgs.sqlite pkgs.python3 ];
    environment.etc."proxy-unrelated".text = "unrelated host data";
    networking.hosts."127.0.0.1" = [ "printer.test" ];
    # Reqwest queries both address families. Answer absent AAAA records locally
    # instead of waiting for an unreachable external resolver in the VM.
    networking.nameservers = [ "127.0.0.1" ];
    services.dnsmasq = {
      enable = true;
      settings = {
        local = "/printer.test/";
        no-resolv = true;
      };
    };
    systemd.tmpfiles.rules = [
      "f /run/proxy-test.env 0600 root root - PROXY_NAMESPACE_TEST=present"
      "f /srv/proxy-unrelated 0644 root root - unrelated"
    ];
    systemd.services.mock-printer = {
      wantedBy = [ "multi-user.target" ];
      serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 ${pkgs.writeText "mock-printer.py" ''
        from http.server import BaseHTTPRequestHandler, HTTPServer
        import socketserver, threading, struct, zlib

        def chunk(kind, data):
            return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

        rows = b"".join(b"\x00" + bytes(0 if x < 8 and y < 8 else 255 for x in range(64)) for y in range(32))
        png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 64, 32, 8, 0, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b"")
        with open("/tmp/mock-printer.png", "wb") as f:
            f.write(png)

        class Control(socketserver.StreamRequestHandler):
            def handle(self):
                try:
                    for line in self.rfile:
                        value = b"SERIAL-1" if b"device.unique_id" in line else b"ZD621" if b"device.product_name" in line else b"V1" if b"appl.name" in line else b"?"
                        self.wfile.write(b'"' + value + b'"\r\n')
                        self.wfile.flush()
                except ConnectionError:
                    pass  # Clients may close after reading the quoted value.

        server = socketserver.ThreadingTCPServer(("127.0.0.1", 9101), Control)
        threading.Thread(target=server.serve_forever, daemon=True).start()

        class Printer(BaseHTTPRequestHandler):
            def authorized(self):
                import os
                if os.path.exists("/run/secrets/expected-token"):
                    with open("/run/secrets/expected-token") as f:
                        expected = f.read()
                    if self.headers.get("Authorization") != "Bearer " + expected:
                        self.send_error(401)
                        return False
                return True

            def do_POST(self):
                if not self.authorized(): return
                self.rfile.read(int(self.headers.get("Content-Length", "0")))
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b'<IMG SRC="/image">')

            def do_GET(self):
                if not self.authorized(): return
                self.send_response(200)
                self.end_headers()
                self.wfile.write(png)

        HTTPServer(("127.0.0.1", 9100), Printer).serve_forever()
      ''}";
    };
  };
in
# NixOS Python test driver: https://nixos.org/manual/nixos/unstable/#sec-nixos-tests
pkgs.testers.runNixOSTest {
  name = "zpl-proxy-api";
  nodes.tcp.imports = [ common ];
  nodes.unix = { lib, ... }: {
    imports = [ common ];
    services.zpl-proxy-api = {
      printers = lib.mkForce { };
      printersFile = "/run/secrets/printers.json";
      unixSocket = "/run/zpl-proxy-api.sock";
      unixSocketGroup = "proxy-clients";
      # The TCP-only firewall option must have no effect in Unix socket mode.
      openFirewall = true;
    };
    users.groups.proxy-clients = { };
    users.users.proxy-client = {
      isNormalUser = true;
      extraGroups = [ "proxy-clients" ];
    };
  };

  testScript = ''
    import json, secrets, shlex
    start_all()
    unix.wait_for_unit("multi-user.target")
    # Model a sops-provisioned root-only file. Generate the value inside the test
    # driver so no credential value exists in a derivation or the Nix store.
    token = secrets.token_hex(24)
    printer_config = {"ZD621": { "url": "http://printer.test:9100/",
                       "control_address": "printer.test:9101", "width": 64, "height": 32,
                       "headers": ["Authorization: Bearer " + token]}}
    def provision(value):
        printer_config["ZD621"]["headers"] = ["Authorization: Bearer " + value]
        unix.succeed("install -d -m 0700 /run/secrets")
        unix.succeed("umask 077; printf %s " + shlex.quote(json.dumps(printer_config)) + " > /run/secrets/next.json; mv /run/secrets/next.json /run/secrets/printers.json")
        unix.succeed("umask 077; printf %s " + shlex.quote(value) + " > /run/secrets/expected-token")
    def check_png(machine):
        machine.succeed("grep -i 'x-zpl-printer-model: ZD621' /tmp/headers")
        machine.succeed("grep -i 'x-zpl-printer-serial: SERIAL-1' /tmp/headers")
        machine.succeed("grep -i 'x-zpl-printer-firmware: V1' /tmp/headers")
        script = "import sqlite3; from pathlib import Path; original=Path('/tmp/mock-printer.png').read_bytes(); response=Path('/tmp/image').read_bytes(); db=sqlite3.connect('/var/lib/zpl-proxy-api/db.sqlite'); assert db.execute('SELECT data FROM pngs').fetchone()[0] == original; assert b'ZPL Source' in response and b'^XA^XZ' in response; assert b'ZPL Printer Configuration' in response; assert response.startswith(original[:-12]) and response.endswith(original[-12:]) and len(response) > len(original)"
        machine.succeed("python3 -c " + shlex.quote(script))

    provision(token)
    for machine, transport in [(tcp, ""), (unix, "--unix-socket /run/zpl-proxy-api.sock")]:
        machine.wait_for_unit("zpl-proxy-api.socket")
        machine.wait_for_unit("mock-printer.service")
        machine.wait_for_unit("dnsmasq.service")
        machine.fail("systemctl is-active --quiet zpl-proxy-api.service")
        curl = f"curl --fail --max-time 30 {transport}"
        machine.succeed(f"{curl} http://127.0.0.1:3000/ | grep -i '<html'")
        machine.wait_for_unit("zpl-proxy-api.service")
        assert machine.succeed("systemctl show -p ExecStart --value zpl-proxy-api.service").find("/run/credentials/") != -1
        # Inspect the actual worker root, not a separately configured sandbox.
        # proc_pid_root(5): https://man7.org/linux/man-pages/man5/proc_pid_root.5.html
        pid = machine.succeed("systemctl show -p MainPID --value zpl-proxy-api.service").strip()
        root = f"/proc/{pid}/root"
        for path in ["/etc/proxy-unrelated", "/srv/proxy-unrelated", "/run/proxy-test.env"]:
            machine.succeed(f"test -f {path}")
            machine.fail(f"cat {root}{path}")
        unrelated_store_path = machine.succeed("readlink -f /etc/proxy-unrelated").strip()
        machine.fail(f"cat {root}{unrelated_store_path}")
        for path in ["/etc/hosts", "/etc/resolv.conf", "/etc/nsswitch.conf", "/etc/ssl/certs/ca-certificates.crt"]:
            machine.succeed(f"test -s {root}{path}")
        machine.succeed(f"grep -zq '^PROXY_NAMESPACE_TEST=present$' /proc/{pid}/environ")
        request = f"{curl} -sS -D /tmp/headers -o /tmp/image --data-urlencode 'zpl=^XA^XZ' http://127.0.0.1:3000/api/printers/ZD621/preview"
        machine.succeed(request)
        machine.succeed("grep -i 'x-zpl-cache: miss' /tmp/headers")
        assert machine.succeed("sqlite3 /var/lib/zpl-proxy-api/db.sqlite 'SELECT name || char(58) || serial || char(58) || firmware FROM printer_requests'").strip() == "ZD621:SERIAL-1:V1"
        check_png(machine)
        machine.succeed("systemctl stop zpl-proxy-api.service")
        machine.wait_for_unit("zpl-proxy-api.socket")
        # A new connection reactivates the worker with the same listener and DB.
        machine.succeed(request)
        machine.succeed("grep -i 'x-zpl-cache: hit' /tmp/headers")
        check_png(machine)
        assert machine.succeed("sqlite3 /var/lib/zpl-proxy-api/db.sqlite 'SELECT COUNT(*) FROM png_requests'").strip() == "2"

    pid = unix.succeed("systemctl show -p MainPID --value zpl-proxy-api.service").strip()
    unix.fail(f"cat /proc/{pid}/root/run/secrets/printers.json")
    unix.fail("su -s /bin/sh nobody -c 'cat /run/credentials/zpl-proxy-api.service/printers.json'")
    unix.fail(f"grep -F {token} /proc/{pid}/cmdline /proc/{pid}/environ")
    unix.fail(f"journalctl -u zpl-proxy-api.service | grep -F {token}")
    unix.fail(f"systemctl cat zpl-proxy-api.service | grep -F {token}")
    unix.succeed("test $(stat -c %a /run/secrets/printers.json) = 600")
    token = secrets.token_hex(24)
    provision(token)
    unix.succeed("systemctl restart zpl-proxy-api.service")
    unix.succeed(request + " --data-urlencode refresh=true")
    unix.succeed("grep -i 'x-zpl-cache: miss' /tmp/headers")
    check_png(unix)

    assert unix.succeed("stat -c '%a %G' /run/zpl-proxy-api.sock").strip() == "660 proxy-clients"
    unix.succeed("su -s /bin/sh proxy-client -c 'curl --fail --unix-socket /run/zpl-proxy-api.sock http://localhost/'")
    unix.fail("su -s /bin/sh nobody -c 'curl --fail --unix-socket /run/zpl-proxy-api.sock http://localhost/'")
    unix.fail("curl --fail --max-time 1 http://127.0.0.1:3000/")
    unix.succeed("systemctl stop zpl-proxy-api.socket")
    unix.fail("systemctl is-active --quiet zpl-proxy-api.service")
    unix.succeed("test ! -e /run/zpl-proxy-api.sock")
    unix.succeed("rm /run/secrets/printers.json; systemctl start zpl-proxy-api.socket")
    unix.fail("systemctl start zpl-proxy-api.service")
    unix.succeed("systemctl stop zpl-proxy-api.socket")
  '';
}
