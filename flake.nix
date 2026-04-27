{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs-stable.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    crane.url = "github:ipetkov/crane";
  };
  outputs =
    {
      self,
      nixpkgs,
      nixpkgs-stable,
      flake-utils,
      fenix,
      crane,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          system = system;
        };
        pkgs-stable = import nixpkgs-stable {
          system = system;
        };

        toolchain = fenix.packages.${system}.stable.toolchain;
        craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            (craneLib.filterCargoSources path type)
            || (builtins.match ".*\.md$" (builtins.baseNameOf path) != null);
        };

        commonArgs = {
          inherit src;
          strictDeps = true;
          nativeBuildInputs = with pkgs; [ pkg-config ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        server = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          pname = "server";
          cargoExtraArgs = "-p server";
        });

        packages =
          with pkgs;
          [
            cargo-info
            cargo-udeps
            pkg-config
            just
            taplo
            imagemagick
            (
              with fenix.packages.${system};
              combine [
                complete.rustc
                complete.rust-src
                complete.cargo
                complete.clippy
                complete.rustfmt
                complete.rust-analyzer
              ]
            )
          ]
          ++ [ pkgs-stable.biome ]; # Add biome from stable outside the 'with pkgs' scope
        libraries = with pkgs; [
          openssl
          glib
          glibc.dev
          libclang
          gcc
        ];
      in
      {
        packages = {
          inherit server;
          default = server;
        };

        devShell = pkgs.mkShell {
          buildInputs = packages ++ libraries;
          LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath libraries}";
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
        };
      }
    );
}
