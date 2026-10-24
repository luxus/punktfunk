# bun2nix 2.1.2 names a non-npmjs package `name@version@@host@@@1`.
# bun 1.4.4 appends `__` and the Wyhash11 of the registry URL when that URL
# is not exactly `https://host/` (a path, a port, or http). Alias the hashed
# name onto the entry bun2nix already fetched so `bun install` stays offline.
{
  lib,
  runCommand,
  python3,
}:
{ deps, npmrc }:
runCommand "bun-cache-registry-aliases" { nativeBuildInputs = [ python3 ]; } ''
  mkdir -p "$out/share"
  cp -a ${deps}/share/bun-cache "$out/share/bun-cache"
  python3 ${./bun-registry-cache.py} "$out/share/bun-cache" ${lib.escapeShellArg "${npmrc}"}
''
