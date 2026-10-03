{ pkgs }:

let
  common = {
    imports = [ ./module.nix ];
    services.zpl-proxy-api = {
      enable = true;
      printerUrl = "http://printer.test:9100/";
      printerSgdAddress = "127.0.0.1:9101";
      environment.OTEL_TRACES_EXPORTER = "none";
      environmentFile = "/run/proxy-test.env";
    };
    environment.systemPackages = [ pkgs.curl pkgs.sqlite pkgs.python3 ];
    environment.etc."proxy-unrelated".text = "unrelated host data";
    networking.hosts."127.0.0.1" = [ "printer.test" ];
    # Supply both address families so the HTTP resolver never needs external
    # DNS for this test hostname. IPv6 connects may fall back to the IPv4 mock.
    networking.hosts."::1" = [ "printer.test" ];
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
            return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))

        png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 0, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(b'\0\0')) + chunk(b'IEND', b"")

        class Sgd(socketserver.StreamRequestHandler):
            def handle(self):
                for line in self.rfile:
                    assert line.startswith(b'! U1 getvar "') and line.endswith(b'"\r\n')
                    name = line.split(b'"')[1]
                    value = {b'device.product_name': b'ZD621', b'device.unique_id': b'TEST-SERIAL', b'appl.name': b'V93.21.33Z', b'head.resolution.in_dpi': b'203'}.get(name, b'?')
                    self.wfile.write(b'"' + value + b'"\r\n')

        sgd = socketserver.TCPServer(("127.0.0.1", 9101), Sgd)
        threading.Thread(target=sgd.serve_forever, daemon=True).start()

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
        request = f"{curl} -sS -D /tmp/headers -o /tmp/image --data-urlencode 'zpl=^XA^XZ' http://127.0.0.1:3000/api/zpl-zd621"
        machine.succeed(request)
        machine.succeed("grep -i 'x-zpl-cache: miss' /tmp/headers")
        machine.succeed("grep -i 'x-zpl-printer-serial: TEST-SERIAL' /tmp/headers")
        machine.succeed("cp /tmp/image /tmp/first-image")
        # Original bytes remain unannotated; returned metadata describes SGD.
        machine.succeed("python3 -c \"import sqlite3; p=open('/tmp/image','rb').read(); raw=sqlite3.connect('/var/lib/zpl-proxy-api/db.sqlite').execute('SELECT data FROM pngs').fetchone()[0]; assert raw.startswith(bytes.fromhex('89504e470d0a1a0a')); assert b'ZPL Source' not in raw; assert b'ZPL Source' in p and b'TEST-SERIAL' in p and b'head.resolution.in_dpi' in p\"")
        machine.succeed("systemctl stop mock-printer.service zpl-proxy-api.service")
        machine.wait_for_unit("zpl-proxy-api.socket")
        # A new connection reactivates the worker with the same listener and DB.
        machine.succeed(request)
        machine.succeed("grep -i 'x-zpl-cache: hit' /tmp/headers")
        machine.succeed("cmp /tmp/first-image /tmp/image")
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
