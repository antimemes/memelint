{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  packages = with pkgs; [
    cargo
    rustc
    clippy
    rustfmt
    rust-analyzer
    mdbook
  ];

  RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";

  STE100_PDF = import ./nix/asd-ste100.nix { inherit pkgs; };
  STE100_ISSUE = "9";

  # Each tree-sitter grammar that nixpkgs has, as <name>.so. `memelint check`
  # loads them at run time.
  MEMELINT_GRAMMARS = pkgs.tree-sitter.withPlugins (p: builtins.attrValues p);

  # `cargo build` puts memelint and memelint-import on PATH.
  shellHook = ''
    export PATH="$PWD/target/debug:$PATH"
  '';
}
