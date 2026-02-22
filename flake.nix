{
  description = "Flang compiler";

  inputs = {
    nixpkgs = {
      url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    };
    crane = {
      url = "github:ipetkov/crane";
    };
    flake-utils = {
      url = "github:numtide/flake-utils";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, crane, flake-utils, rust-overlay, advisory-db, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        inherit (pkgs) lib;

        rustToolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
          extensions = [ "rust-analyzer" "rust-src" ];
        });

        craneLib = ((crane.mkLib pkgs).overrideToolchain rustToolchain);

        src = lib.cleanSourceWith
          {
            src = ./.;
            filter = path: type:
              (craneLib.filterCargoSources path type) ||
              (lib.hasSuffix "\.md" path) ||
              (lib.hasSuffix "\.asm" path)
            ;
          };

        RUSTFLAGS = "-Z threads=8 -C target-cpu=native";
        args = {
          inherit src;
          strictDeps = true;
          buildInputs = [ ];

          inherit RUSTFLAGS;
        };

        cargoArtifacts = craneLib.buildDepsOnly args;
        crate = craneLib.buildPackage (
          args // {
            inherit cargoArtifacts;

            nativeBuildInputs = [ pkgs.makeWrapper ];

            postInstall = ''
              wrapProgram $out/bin/flang --prefix PATH : ${lib.makeBinPath [ pkgs.nasm pkgs.binutils ]}
            '';
          }
        );
      in
      {
        checks = {
          inherit crate;

          clippy = craneLib.cargoClippy (
            args // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          fmt = craneLib.cargoFmt {
            inherit src;
          };

          audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };
        };

        packages.default = crate;

        apps.default = flake-utils.lib.mkApp {
          drv = crate;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = with pkgs; [
            rustToolchain

            cargo-flamegraph
            cargo-outdated

            nasm
            binutils

            nasmfmt
          ];

          inherit RUSTFLAGS;
        };
      }
    );
}
