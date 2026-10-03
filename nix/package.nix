{ lib, rustPlatform, pkg-config, sqlite, cacert }:

rustPlatform.buildRustPackage {
  pname = "zpl-proxy-api";
  version = (builtins.fromTOML (builtins.readFile ../zpl-proxy-api/Cargo.toml)).package.version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../LICENSE
      ../zpl
      ../zpl-cmd
      ../zpl-render-api
      ../zpl-font-extract
      ../zpl-wasm
      ../raster-diff
      ../zebra-sgd
      ../zebra-firmware
      ../zebra-http-api
      ../zpl-proxy-api
    ];
  };
  # Use the workspace lockfile checksums for reproducible vendoring.
  # https://nixos.org/manual/nixpkgs/unstable/#importing-a-cargo.lock-file
  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [ "-p" "zpl-proxy-api" ];
  cargoTestFlags = [ "-p" "zpl-proxy-api" ];

  nativeBuildInputs = [ pkg-config ];
  buildInputs = [ sqlite ];

  # Reqwest initializes its system trust store even for loopback HTTP tests.
  SSL_CERT_FILE = "${cacert}/etc/ssl/certs/ca-bundle.crt";

  postInstall = ''
    mkdir -p $out/share/zpl-proxy-api
    cp -r zpl-proxy-api/assets zpl-proxy-api/migrations $out/share/zpl-proxy-api/
  '';

  meta = {
    description = "Printer-backed ZPL rendering proxy with a persistent SQLite cache";
    license = lib.licenses.osl3;
    mainProgram = "zpl-proxy-api";
    platforms = lib.platforms.linux;
  };
}
