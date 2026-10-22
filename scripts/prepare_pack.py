#!/usr/bin/env python3
"""Prepare un pack modele ptts a partir des poids Kyutai.

Kyutai expose `flow_lm.bos_before_voice`, que `ptts` attend sous le nom
`flow_lm.condition_provider.conditioners.speaker_wavs.sink` (cf. le `remap_key` de
`kyutai-labs/pocket-tts/rust/src/weights.rs`, absent de `ptts::loader::remap_key`).

`ptts::loader::load_weights` n'accepte qu'un seul fichier de poids et applique son
propre `remap_key`, donc le renommage est fait ici, une fois, hors ligne. Le fichier
est reecrit : l'en-tete JSON change de taille, donc les donnees se decalent de
quelques octets, mais les `data_offsets` du header sont relatifs au debut des donnees
et restent valides.

    python prepare_pack.py <source.safetensors> <dest.safetensors>
"""

import json
import struct
import sys

RENAMES = {
    "flow_lm.bos_before_voice": "flow_lm.condition_provider.conditioners.speaker_wavs.sink",
}


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    source, dest = sys.argv[1:]

    with open(source, "rb") as handle:
        (header_len,) = struct.unpack("<Q", handle.read(8))
        header = json.loads(handle.read(header_len))
        data_start = 8 + header_len

        renamed = {}
        for key, value in header.items():
            renamed[RENAMES.get(key, key)] = value
        new_header = json.dumps(renamed, separators=(",", ":")).encode("utf-8")

        with open(dest, "wb") as out:
            out.write(struct.pack("<Q", len(new_header)))
            out.write(new_header)
            handle.seek(data_start)
            remaining = handle.read(1 << 20)
            while remaining:
                out.write(remaining)
                remaining = handle.read(1 << 20)

    print(f"wrote {dest} ({len(new_header)} bytes of header, {len(renamed)} tensors)")
    for old, new in RENAMES.items():
        print(f"  renamed {old} -> {new}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
