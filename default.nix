# The memelint package without flakes, with nixpkgs from flake.lock.
let
  sources = import ./nix/locked-sources.nix { };
  pkgs = import sources.nixpkgs { };
in
(import ./nix/package.nix { inherit pkgs; }).default
