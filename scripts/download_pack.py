#!/usr/bin/env python3
"""Telecharge les poids du checkpoint Pocket TTS pour une langue.

Le depot `kyutai/pocket-tts-without-voice-cloning` contient, pour chaque
langue, les poids du modele (`model.safetensors`), son tokenizer, et les
embeddings de voix (`embeddings/<voix>.safetensors`). Ces embeddings sont les
etats de conditionnement precalcules : le pack mobile n'a pas besoin d'un
prompt audio, contrairement au pack ONNX.

    python download_pack.py --config french_24l --out pack

Les fichiers sont places dans le dossier de pack avec les noms que
`ptts::checkpoint::Checkpoint::locate` attend :
`model.safetensors`, `tokenizer.json`, `embeddings/<voix>.safetensors`.
"""

import argparse
import os
import sys

REPO = "kyutai/pocket-tts-without-voice-cloning"
REV = "1e08e6a23401048648a9fdcfde2f89348215c2a7"

# Ce que chaque langue apporte, et comment le nommer dans le pack.
FILES = {
    "french_24l": [
        ("languages/french_24l/model.safetensors", "model.safetensors"),
        ("languages/french_24l/tokenizer.json", "tokenizer.json"),
        ("languages/french_24l/embeddings/estelle.safetensors", "embeddings/estelle.safetensors"),
    ],
    "french": [
        ("languages/french/model.safetensors", "model.safetensors"),
        ("languages/french/tokenizer.json", "tokenizer.json"),
        ("languages/french/embeddings/estelle.safetensors", "embeddings/estelle.safetensors"),
    ],
}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", default="french_24l", choices=sorted(FILES))
    parser.add_argument("--out", default="pack", help="dossier du pack")
    parser.add_argument("--repo", default=REPO)
    parser.add_argument("--rev", default=REV)
    args = parser.parse_args()

    try:
        from huggingface_hub import hf_hub_download
    except ImportError:
        print("huggingface_hub est requis : pip install huggingface_hub", file=sys.stderr)
        return 1

    os.makedirs(args.out, exist_ok=True)
    for src, dst in FILES[args.config]:
        out = os.path.join(args.out, dst)
        os.makedirs(os.path.dirname(out) or ".", exist_ok=True)
        print(f"telechargement {src}", flush=True)
        path = hf_hub_download(args.repo, src, revision=args.rev)
        # On deplace le fichier plutot que de le copier : le cache de
        # huggingface_hub garde sa version propre, le pack garde la sienne.
        os.replace(path, out)
        print(f"  ok {dst} ({os.path.getsize(out)} octets)", flush=True)
    print("DONE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
