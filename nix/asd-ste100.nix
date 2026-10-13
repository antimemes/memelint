# ASD-STE100 Issue 9 (2025-01-15). Copyright ASD; free to download, not free
# to redistribute. It is fetched into the Nix store for local use only.
# Nothing derived from it is committed to this repository or built into the
# memelint binary.
{ pkgs }:

pkgs.fetchurl {
  name = "ASD-STE100_ISSUE9.pdf";
  url = "https://www.asd-ste100.org/assets/files/ASD-STE100_ISSUE9.pdf";
  hash = "sha256-0fTqnnzW5GtHqpBXIJ+Z54wOnPxOJ6WweJWwXBoWZDE=";
}
