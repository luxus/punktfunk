#!/usr/bin/env python3
"""Point bun 1.4.4's hashed registry cache names at bun2nix's entries.

bun2nix 2.1.2 stores a non-default registry package at
``name@version@@host@@@1``. bun 1.4.4 uses that name only for
``https://host/``. A registry URL with a path, a port, or http gets
``@@host__<wyhash>@@@1`` instead, and an offline install misses.

The hash is Wyhash11 (seed 0) of the registry URL as ``.npmrc`` writes it.
"""

from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path
from urllib.parse import urlparse

_PRIMES = (
    0xA0761D6478BD642F,
    0xE7037ED1A0B428DB,
    0x8EBC6AF09C88C6E3,
    0x589965CC75374CC3,
    0x1D8E4E27C47D124F,
)
_MASK = (1 << 64) - 1


def _u64(n: int) -> int:
    return n & _MASK


def _read(data: bytes, n: int, off: int = 0) -> int:
    return int.from_bytes(data[off : off + n], "little")


def _read8(data: bytes, off: int = 0) -> int:
    return _u64((_read(data, 4, off) << 32) | _read(data, 4, off + 4))


def _mum(a: int, b: int) -> int:
    r = a * b
    return _u64((r >> 64) ^ r)


def _mix0(a: int, b: int, seed: int) -> int:
    return _mum(a ^ seed ^ _PRIMES[0], b ^ seed ^ _PRIMES[1])


def _mix1(a: int, b: int, seed: int) -> int:
    return _mum(a ^ seed ^ _PRIMES[2], b ^ seed ^ _PRIMES[3])


def wyhash11(data: bytes, seed: int = 0) -> int:
    """Bun's ``Wyhash11::hash`` (the zig 0.11 stateless form)."""
    aligned = len(data) - (len(data) % 32)
    off = 0
    while off < aligned:
        block = data[off : off + 32]
        seed = _u64(
            _mix0(_read(block, 8, 0), _read(block, 8, 8), seed)
            ^ _mix1(_read(block, 8, 16), _read(block, 8, 24), seed)
        )
        off += 32
    rest = data[aligned:]
    n = len(rest)

    def rb(k: int, at: int = 0) -> int:
        return _read(rest, k, at)

    def r8(at: int = 0) -> int:
        return _read8(rest, at)

    if n == 0:
        mixed = seed
    elif n == 1:
        mixed = _mix0(rb(1), _PRIMES[4], seed)
    elif n == 2:
        mixed = _mix0(rb(2), _PRIMES[4], seed)
    elif n == 3:
        mixed = _mix0(_u64((rb(2) << 8) | rb(1, 2)), _PRIMES[4], seed)
    elif n == 4:
        mixed = _mix0(rb(4), _PRIMES[4], seed)
    elif n == 5:
        mixed = _mix0(_u64((rb(4) << 8) | rb(1, 4)), _PRIMES[4], seed)
    elif n == 6:
        mixed = _mix0(_u64((rb(4) << 16) | rb(2, 4)), _PRIMES[4], seed)
    elif n == 7:
        mixed = _mix0(_u64((rb(4) << 24) | (rb(2, 4) << 8) | rb(1, 6)), _PRIMES[4], seed)
    elif n == 8:
        mixed = _mix0(r8(), _PRIMES[4], seed)
    elif n == 9:
        mixed = _mix0(r8(), rb(1, 8), seed)
    elif n == 10:
        mixed = _mix0(r8(), rb(2, 8), seed)
    elif n == 11:
        mixed = _mix0(r8(), _u64((rb(2, 8) << 8) | rb(1, 10)), seed)
    elif n == 12:
        mixed = _mix0(r8(), rb(4, 8), seed)
    elif n == 13:
        mixed = _mix0(r8(), _u64((rb(4, 8) << 8) | rb(1, 12)), seed)
    elif n == 14:
        mixed = _mix0(r8(), _u64((rb(4, 8) << 16) | rb(2, 12)), seed)
    elif n == 15:
        mixed = _mix0(r8(), _u64((rb(4, 8) << 24) | (rb(2, 12) << 8) | rb(1, 14)), seed)
    elif n == 16:
        mixed = _mix0(r8(), r8(8), seed)
    elif n == 17:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(rb(1, 16), _PRIMES[4], seed)
    elif n == 18:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(rb(2, 16), _PRIMES[4], seed)
    elif n == 19:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(_u64((rb(2, 16) << 8) | rb(1, 18)), _PRIMES[4], seed)
    elif n == 20:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(rb(4, 16), _PRIMES[4], seed)
    elif n == 21:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(_u64((rb(4, 16) << 8) | rb(1, 20)), _PRIMES[4], seed)
    elif n == 22:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(_u64((rb(4, 16) << 16) | rb(2, 20)), _PRIMES[4], seed)
    elif n == 23:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(
            _u64((rb(4, 16) << 24) | (rb(2, 20) << 8) | rb(1, 22)), _PRIMES[4], seed
        )
    elif n == 24:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(r8(16), _PRIMES[4], seed)
    elif n == 25:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(r8(16), rb(1, 24), seed)
    elif n == 26:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(r8(16), rb(2, 24), seed)
    elif n == 27:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(r8(16), _u64((rb(2, 24) << 8) | rb(1, 26)), seed)
    elif n == 28:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(r8(16), rb(4, 24), seed)
    elif n == 29:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(r8(16), _u64((rb(4, 24) << 8) | rb(1, 28)), seed)
    elif n == 30:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(r8(16), _u64((rb(4, 24) << 16) | rb(2, 28)), seed)
    elif n == 31:
        mixed = _mix0(r8(), r8(8), seed) ^ _mix1(
            _u64((rb(4, 24) << 24) | (rb(2, 28) << 8) | rb(1, 30)), _PRIMES[4], seed
        )
    else:
        raise ValueError(f"wyhash remainder {n}")
    return _mum(_u64(mixed) ^ (aligned + n), _PRIMES[4])


def _hex16(n: int) -> str:
    return f"{n:016x}"


def registry_token(url: str) -> str | None:
    """Cache-folder token bun 1.4.4 appends after ``@@``, or None for ``https://host/``."""
    parsed = urlparse(url)
    host = parsed.hostname or ""
    if not host or parsed.scheme not in ("http", "https"):
        return None
    path = parsed.path or ""
    visible = host[:12] if len(host) > 32 else host
    needs_hash = (
        len(visible) < len(host) or parsed.scheme != "https" or parsed.port is not None or len(path) > 1
    )
    if not needs_hash:
        return None
    return f"{visible}__{_hex16(wyhash11(url.encode()))}"


def scoped_registries(text: str) -> list[tuple[str, str]]:
    found: list[tuple[str, str]] = []
    for raw in text.splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line or line.startswith(";") or not line.startswith("@"):
            continue
        key, sep, value = line.partition(":registry=")
        if not sep or not key.startswith("@") or "/" in key:
            continue
        url = value.strip().strip('"').strip("'")
        if url:
            found.append((key, url))
    return found


def alias_cache(cache: Path, npmrc: str) -> tuple[list[str], list[str]]:
    """Alias bun2nix's ``@@host@@@1`` entries. Returns (created, scopes with no match)."""
    created: list[str] = []
    missed: list[str] = []
    for scope, url in scoped_registries(npmrc):
        token = registry_token(url)
        if token is None:
            continue
        host = urlparse(url).hostname or ""
        suffix = f"@@{host}@@@1"
        scope_dir = cache / scope
        matched = False
        if scope_dir.is_dir():
            for entry in scope_dir.iterdir():
                name = entry.name
                if not name.endswith(suffix):
                    continue
                new_name = name[: -len(suffix)] + f"@@{token}@@@1"
                dest = scope_dir / new_name
                matched = True
                if dest.exists() or dest.is_symlink():
                    continue
                target = os.readlink(entry) if entry.is_symlink() else str(entry.resolve())
                dest.symlink_to(target)
                created.append(f"{scope}/{new_name}")
        if not matched:
            missed.append(scope)
    return created, missed


def _self_test() -> None:
    assert wyhash11(b"beta.9") == 0x73C5C46324E78B9B
    url = "https://git.unom.io/api/packages/unom/npm/"
    assert registry_token(url) == "git.unom.io__4edaa278c5298c3a"
    assert registry_token("https://registry.npmjs.org/") is None
    assert registry_token("https://npm.pkg.github.com") is None
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        pkg = root / "@unom"
        pkg.mkdir()
        entry = pkg / "style@0.4.4@@git.unom.io@@@1"
        entry.symlink_to("/nix/store/example-style")
        made, missed = alias_cache(root, f"@unom:registry={url}\n")
        assert missed == []
        assert made == ["@unom/style@0.4.4@@git.unom.io__4edaa278c5298c3a@@@1"]
        assert os.readlink(pkg / "style@0.4.4@@git.unom.io__4edaa278c5298c3a@@@1") == "/nix/store/example-style"
        again, missed_again = alias_cache(root, f"@unom:registry={url}\n")
        assert again == [] and missed_again == []
        plain, plain_missed = alias_cache(root, "@other:registry=https://npm.pkg.github.com\n")
        assert plain == [] and plain_missed == []


def main(argv: list[str]) -> int:
    if argv == ["--self-test"]:
        _self_test()
        print("bun-registry-cache: self-test ok")
        return 0
    if len(argv) != 2:
        print("usage: bun-registry-cache.py <cache-dir> <npmrc> | --self-test", file=sys.stderr)
        return 2
    cache = Path(argv[0])
    npmrc = Path(argv[1]).read_text()
    created, missed = alias_cache(cache, npmrc)
    for name in created:
        print(f"bun-registry-cache: {name}")
    if missed:
        print(
            "bun-registry-cache: no @@host@@@1 entry for " + ", ".join(missed),
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
