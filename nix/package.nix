# The memelint packages.
#
# `default` is the checker, with a wrapper that gives it the ASD-STE100
# vocabulary and each tree-sitter grammar in nixpkgs. The vocabulary is a
# different derivation, which runs the importer on the PDF. Thus the checker
# binary contains no text from the standard.
{ pkgs }:

let
  inherit (pkgs) lib;

  ste100Pdf = import ./asd-ste100.nix { inherit pkgs; };
  grammars = pkgs.tree-sitter.withPlugins (p: builtins.attrValues p);
  version = (lib.importTOML ../Cargo.toml).workspace.package.version;

  src = lib.cleanSourceWith {
    src = ../.;
    filter = path: type:
      let name = baseNameOf path;
      in lib.cleanSourceFilter path type
        && !(builtins.elem name [ "target" ".direnv" ]);
  };

  crate = pname: extraFlags:
    pkgs.rustPlatform.buildRustPackage {
      inherit pname version src;
      cargoLock.lockFile = ../Cargo.lock;
      cargoBuildFlags = [ "-p" pname ] ++ extraFlags;
      cargoTestFlags = [ "-p" pname ] ++ extraFlags;
      # The unit tests load grammars from here.
      MEMELINT_GRAMMARS = grammars;
      meta.mainProgram = pname;
    };

  memelint-import = crate "memelint-import" [ ];

  # No bundled grammars: the wrapper supplies the directory.
  memelint-unwrapped = crate "memelint" [ "--no-default-features" ];

  vocabulary = pkgs.runCommand "memelint-vocabulary-asd-ste100"
    {
      nativeBuildInputs = [ memelint-import ];
      meta.description = "The ASD-STE100 vocabulary for memelint, made from the PDF of ASD";
    } ''
    mkdir -p $out
    memelint-import asd-ste100 --pdf ${ste100Pdf} --out $out/asd-ste100.json
  '';

  memelint = pkgs.symlinkJoin {
    name = "memelint-${version}";
    paths = [ memelint-unwrapped ];
    nativeBuildInputs = [ pkgs.makeWrapper ];
    postBuild = ''
      wrapProgram $out/bin/memelint \
        --set-default MEMELINT_VOCAB ${vocabulary}/asd-ste100.json \
        --set-default MEMELINT_GRAMMARS ${grammars}
    '';
    meta.mainProgram = "memelint";
  };
in
{
  default = memelint;
  inherit memelint memelint-import vocabulary grammars;
  unwrapped = memelint-unwrapped;
}
