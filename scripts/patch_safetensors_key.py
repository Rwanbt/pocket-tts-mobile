#!/usr/bin/env python3
"""Renomme une cle de tenseur dans l'en-tete safetensors, sans deplacer les donnees.

Le pack Kyutai expose `flow_lm.bos_before_voice`, que `ptts` attend sous le nom
`flow_lm.condition_provider.conditioners.speaker_wavs.sink` (cf. le `remap_key` de
`kyutai-labs/pocket-tts/rust/src/weights.rs`, absent de `ptts::loader::remap_key`).

L'en-tete JSON est reecrit a la meme taille (comblee par des espaces) pour que le
debut des donnees, et donc tous les `data_offsets`, restent valides : le fichier
n'est pas recopie.

    python patch_safetensors_key.py <file> <old-key> <new-key>
"""

import json
import struct
import sys


def main() -> int:
    if len(sys.argv) != 4:
        print(__doc__)
        return 2
    path, old_key, new_key = sys.argv[1:]

    with open(path, "rb") as handle:
        raw_len = handle.read(8)
        (header_len,) = struct.unpack("<Q", raw_len)
        header = handle.read(header_len)

    meta = json.loads(header)
    if old_key not in meta:
        print(f"error: {old_key!r} is not in {path}")
        return 1
    if new_key in meta:
        print(f"error: {new_key!r} is already in {path}")
        return 1

    # On renomme en conservant l'ordre des cles, puis on resserialize.
    renamed = {}
    for key, value in meta.items():
        renamed[new_key if key == old_key else key] = value
    new_header = json.dumps(renamed, separators=(",", ":")).encode("utf-8")
    if len(new_header) > header_len:
        print(
            f"error: the renamed header is {len(new_header)} bytes, "
            f"larger than the {header_len} available"
        )
        return 1

    # Le format safetensors tolere des espaces de fin dans l'en-tete JSON.
    new_header += b" " * (header_len - len(new_header))

    with open(path, "r+b") as handle:
        handle.seek(8)
        handle.write(new_header)

    print(f"patched {path}: {old_key} -> {new_key} (header {header_len} bytes, unchanged size)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
