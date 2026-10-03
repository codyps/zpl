{ pkgs }:

let
  common = {
    imports = [ ./module.nix ];
    services.zpl-proxy-api = {
      enable = true;
      printers = [{
        name = "ZD621";
        url = "http://printer.test:9100/";
        control_address = "printer.test:9101";
        width = 64;
        height = 32;
      }];
      environment.OTEL_TRACES_EXPORTER = "none";
      environmentFile = "/run/proxy-test.env";
    };
    environment.systemPackages = [ pkgs.curl pkgs.sqlite ];
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
                        value = b"SERIAL-1" if b"device.unique_id" in line else b"V1"
                        self.wfile.write(b'"' + value + b'"\r\n')
                        self.wfile.flush()
                except ConnectionError:
                    pass  # Clients may close after reading the quoted value.

        server = socketserver.ThreadingTCPServer(("127.0.0.1", 9101), Control)
        threading.Thread(target=server.serve_forever, daemon=True).start()

        class Printer(BaseHTTPRequestHandler):
            def do_POST(self):
                self.rfile.read(int(self.headers.get("Content-Length", "0")))
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b'<IMG SRC="/image">')

            def do_GET(self):
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
  nodes.unix = {
    imports = [ common ];
    services.zpl-proxy-api = {
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
    start_all()
    for machine, transport in [(tcp, ""), (unix, "--unix-socket /run/zpl-proxy-api.sock")]:
        machine.wait_for_unit("zpl-proxy-api.socket")
        machine.wait_for_unit("mock-printer.service")
        machine.wait_for_unit("dnsmasq.service")
        machine.fail("systemctl is-active --quiet zpl-proxy-api.service")
        curl = f"curl --fail --max-time 30 {transport}"
        machine.succeed(f"{curl} http://127.0.0.1:3000/ | grep -i '<html'")
        machine.wait_for_unit("zpl-proxy-api.service")
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
        machine.succeed("cmp /tmp/image /tmp/mock-printer.png")
        machine.succeed("systemctl stop zpl-proxy-api.service")
        machine.wait_for_unit("zpl-proxy-api.socket")
        # A new connection reactivates the worker with the same listener and DB.
        machine.succeed(request)
        machine.succeed("grep -i 'x-zpl-cache: hit' /tmp/headers")
        machine.succeed("cmp /tmp/image /tmp/mock-printer.png")
        assert machine.succeed("sqlite3 /var/lib/zpl-proxy-api/db.sqlite 'SELECT COUNT(*) FROM png_requests'").strip() == "2"

    assert unix.succeed("stat -c '%a %G' /run/zpl-proxy-api.sock").strip() == "660 proxy-clients"
    unix.succeed("su -s /bin/sh proxy-client -c 'curl --fail --unix-socket /run/zpl-proxy-api.sock http://localhost/'")
    unix.fail("su -s /bin/sh nobody -c 'curl --fail --unix-socket /run/zpl-proxy-api.sock http://localhost/'")
    unix.fail("curl --fail --max-time 1 http://127.0.0.1:3000/")
    unix.succeed("systemctl stop zpl-proxy-api.socket")
    unix.fail("systemctl is-active --quiet zpl-proxy-api.service")
    unix.succeed("test ! -e /run/zpl-proxy-api.sock")
  '';
}
