{
  nixConfig = {
    # Public endpoints and keys: https://cachix.org/api/v1/cache/<cache-name>
    extra-substituters = [
      "https://codyps.cachix.org"
      "https://nix-community.cachix.org"
    ];
    extra-trusted-public-keys = [
      "codyps.cachix.org-1:T2SgQFUIPVsszIqt74ku8XhkfVDVm3sVzn4qOfUoEFY="
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };

  inputs = {
    nixpkgs.url = "nixpkgs/nixpkgs-unstable";
    nixpkgs-intel-darwin.url = "github:NixOS/nixpkgs/nixpkgs-26.05-darwin";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, flake-utils, nixpkgs, nixpkgs-intel-darwin }:
    {
      nixosModules.zpl-proxy-api = import ./nix/module.nix;
      nixosModules.default = self.nixosModules.zpl-proxy-api;
    } // flake-utils.lib.eachDefaultSystem (system:
      let
        # Unstable no longer supports Intel macOS.
        nixpkgsForSystem = if system == "x86_64-darwin" then nixpkgs-intel-darwin else nixpkgs;
        pkgs = ((import nixpkgsForSystem) {
          inherit system;
          config.allowDeprecatedx86_64Darwin = true;
        });
        lib = pkgs.lib;
        stdenv = pkgs.stdenv;
        # Use the versioned Nixpkgs attribute. The Intel Darwin input only has
        # versions through 0.2.126, so build the newer expression with its packages.
        wasmBindgenCli = pkgs.wasm-bindgen-cli_0_2_128 or
          (pkgs.callPackage "${nixpkgs}/pkgs/by-name/wa/wasm-bindgen-cli_0_2_128/package.nix" { });
        # Unstable's release-plz requires Rust >= 1.96; Intel macOS defaults to
        # 1.95. Use its newer packaged toolchain only for building release-plz.
        releasePlz = if system == "x86_64-darwin" then
          pkgs.callPackage "${nixpkgs}/pkgs/by-name/re/release-plz/package.nix" {
            rustPlatform = pkgs.rustPackages_1_97.rustPlatform;
          }
        else
          pkgs.release-plz;
      in
      {
        formatter = pkgs.nixpkgs-fmt;

        packages = lib.optionalAttrs stdenv.hostPlatform.isLinux {
          zpl-proxy-api = pkgs.callPackage ./nix/package.nix { };
          default = self.packages.${system}.zpl-proxy-api;
        };

        checks = lib.optionalAttrs stdenv.hostPlatform.isLinux {
          zpl-proxy-api = import ./nix/test.nix { inherit pkgs; };
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustc
            cargo
            rustfmt
            taplo
            clippy
            rust-analyzer
            bacon

            releasePlz
            cargo-outdated
            cargo-udeps
            cargo-audit
            diesel-cli
            sqlite
            # Match zpl-wasm/Cargo.toml; the unversioned CLI can be newer.
            wasmBindgenCli
            lld
          ] ++ lib.optionals stdenv.hostPlatform.isDarwin [
            xcbuild.xcrun
          ];

          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";

          shellHook = ''
            # Nix prepends Cargo to PATH; keep the installed mbx shim in front.
            # mbx removes its own directory when resolving the underlying Cargo.
            ${if stdenv.hostPlatform.isDarwin then ''
              mbx_shim_dir="$HOME/Library/Application Support/mbx/bin"
            '' else ''
              mbx_shim_dir="''${XDG_DATA_HOME:-$HOME/.local/share}/mbx/bin"
            ''}
            if [ -x "$mbx_shim_dir/cargo" ] && [ -f "$mbx_shim_dir/mbx-target" ]; then
              export PATH="$mbx_shim_dir:$PATH"
            fi
            unset mbx_shim_dir

            export ROOT_PATH="$(git rev-parse --show-toplevel)"
            export DATABASE_URL="$ROOT_PATH/_db/db.sqlite"
          '';
        };
      }
    );
}
