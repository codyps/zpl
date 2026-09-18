{ pkgs }:

let
  common = {
    imports = [ ./module.nix ];
    services.zpl-proxy-api = {
      enable = true;
      printerUrl = "http://printer.test:9100/";
      environment.OTEL_TRACES_EXPORTER = "none";
      environmentFile = "/run/proxy-test.env";
    };
    environment.systemPackages = [ pkgs.curl pkgs.sqlite ];
    environment.etc."proxy-unrelated".text = "unrelated host data";
    networking.hosts."127.0.0.1" = [ "printer.test" ];
    systemd.tmpfiles.rules = [
      "f /run/proxy-test.env 0600 root root - PROXY_NAMESPACE_TEST=present"
      "f /srv/proxy-unrelated 0644 root root - unrelated"
    ];
    systemd.services.mock-printer = {
      wantedBy = [ "multi-user.target" ];
      serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 ${pkgs.writeText "mock-printer.py" ''
        from http.server import BaseHTTPRequestHandler, HTTPServer

        class Printer(BaseHTTPRequestHandler):
            def do_POST(self):
                self.rfile.read(int(self.headers.get("Content-Length", "0")))
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b'<IMG SRC="/image">')

            def do_GET(self):
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b'mock PNG bytes')

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
        assert machine.succeed("cat /tmp/image") == "mock PNG bytes"
        machine.succeed("systemctl stop mock-printer.service zpl-proxy-api.service")
        machine.wait_for_unit("zpl-proxy-api.socket")
        # A new connection reactivates the worker with the same listener and DB.
        machine.succeed(request)
        machine.succeed("grep -i 'x-zpl-cache: hit' /tmp/headers")
        assert machine.succeed("cat /tmp/image") == "mock PNG bytes"
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
