# Without flakes, the root inputs come from the same lock file that the flake
# uses. This does GitHub and tarball inputs only, not all that a flake can do.
{ lockFile ? ../flake.lock }:
let
  lock = builtins.fromJSON (builtins.readFile lockFile);
  fetchInput = name: nodeName:
    if !builtins.isString nodeName then
      throw "locked-sources: root input '${name}' must reference a lock node directly"
    else
      let
        locked = lock.nodes.${nodeName}.locked;
        url =
          if locked.type == "tarball" then locked.url
          else if locked.type == "github" && (locked.host or "github.com") == "github.com" then
            "https://github.com/${locked.owner}/${locked.repo}/archive/${locked.rev}.tar.gz"
          else throw "locked-sources: unsupported source type for '${name}': ${locked.type}";
      in
      if (locked.dir or "") != "" then
        throw "locked-sources: source subdirectories are not supported for '${name}'"
      else
        builtins.fetchTarball { inherit url; sha256 = locked.narHash; };
in
if lock.version != 7 then
  throw "locked-sources: unsupported flake.lock version ${toString lock.version}"
else
  builtins.mapAttrs fetchInput lock.nodes.${lock.root}.inputs
