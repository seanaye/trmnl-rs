{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs-stable.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };
  outputs =
    {
      self,
      nixpkgs,
      nixpkgs-stable,
      flake-utils,
      fenix,
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
        packages =
          with pkgs;
          [
            cargo-info
            cargo-udeps
            pkg-config
            just
            taplo
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
        devShell = pkgs.mkShell {
          buildInputs = packages ++ libraries;
          LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath libraries}";
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
        };
      }
    );
}
