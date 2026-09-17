{ pkgs }:

# NixOS Python test driver: https://nixos.org/manual/nixos/unstable/#sec-nixos-tests
pkgs.testers.runNixOSTest {
  name = "zpl-proxy-api";
  nodes.machine = { ... }: {
    imports = [ ./module.nix ];
    services.zpl-proxy-api = {
      enable = true;
      printerUrl = "http://127.0.0.1:9100/";
      environment.OTEL_TRACES_EXPORTER = "none";
    };
    environment.systemPackages = [ pkgs.curl pkgs.sqlite ];
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

  testScript = ''
    machine.start()
    machine.wait_for_unit("zpl-proxy-api.service")
    machine.wait_for_unit("mock-printer.service")
    machine.wait_for_open_port(3000)
    machine.succeed("curl --fail http://127.0.0.1:3000/ | grep -i '<html'")
    request = "curl --fail -sS -D /tmp/headers -o /tmp/image --data-urlencode 'zpl=^XA^XZ' http://127.0.0.1:3000/api/zpl-zd621"
    machine.succeed(request)
    machine.succeed("grep -i 'x-zpl-cache: miss' /tmp/headers")
    assert machine.succeed("cat /tmp/image") == "mock PNG bytes"
    machine.succeed("systemctl stop mock-printer.service")
    machine.succeed("systemctl restart zpl-proxy-api.service")
    machine.wait_for_open_port(3000)
    machine.succeed(request)
    machine.succeed("grep -i 'x-zpl-cache: hit' /tmp/headers")
    assert machine.succeed("cat /tmp/image") == "mock PNG bytes"
    assert machine.succeed("sqlite3 /var/lib/zpl-proxy-api/db.sqlite 'SELECT COUNT(*) FROM png_requests'").strip() == "2"
  '';
}
