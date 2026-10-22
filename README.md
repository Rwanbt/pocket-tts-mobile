# Pocket TTS Mobile (Rust)

> **Statut** : Work in Progress (Phase 0)
> **Objectif** : Porter le runtime Rust officiel de Pocket TTS (Kyutai) sur Android pour Unifia.
> **Plan** : [PLAN-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md](../../projects/unifia/operations/PLAN-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md)

---

## 📌 À propos

Ce projet est un **fork adaptatif** du runtime Rust de [Pocket TTS](https://github.com/kyutai-labs/pocket-tts) (Kyutai), conçu pour fonctionner sur **Android** (ARM64) dans le cadre de l'application [Unifia](https://github.com/Rwanbt/unifia).

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Unifia (Tauri)                              │
├─────────────────────────────────────────────────────────────┤
│  packages/mobile/src-tauri/src/voice/pocket_tts.rs              │
│       (Appels FFI via libloading)                              │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  libpocket_tts_rust.so (ce projet)                             │
│  ┌─────────────────────────────────────────────────────────┐│
│  │  ffi.rs (ABI C : ptt_create, ptt_stream_start, ...)       ││
│  └─────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────┐│
│  │  Adaptateur Kyutai (config.rs, text.rs, model.rs, ...)     ││
│  └─────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────┐│
│  │  ptts (xn-ptts fork) : Moteur d'inférence Rust              ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Packs modèles (safetensors/GGUF + tokenizer.json + config)    │
└─────────────────────────────────────────────────────────────┘
```

### Pourquoi ce fork ?

1. **Portabilité Android** : Le runtime officiel `pocket-tts` n'a pas de build Android. Ce projet comble ce vide.
2. **Parité avec le desktop** : Réutilisation de l'adaptateur Kyutai pour conserver les réglages spécifiques (ex: `frames_after_eos=8` pour le français).
3. **Upstreamable** : Le code est conçu pour être proposé en amont à Kyutai (`pocket-tts`) ou Gradium (`xn-ptts`).

---

## 🚀 Installation

### Prérequis

- **Rust** : [Installer Rust](https://www.rust-lang.org/tools/install) (version ≥ 1.70).
- **cargo-ndk** : Pour la cross-compilation Android.
  ```bash
  cargo install cargo-ndk
  ```
- **Android NDK** : [Télécharger NDK r26b](https://developer.android.com/ndk/downloads) et définir `ANDROID_NDK_HOME`.
- **Outils Android** : `adb` pour le déploiement sur appareil.

### Build pour Android (ARM64)

```bash
# Installer cargo-ndk
export ANDROID_NDK_HOME=/chemin/vers/ndk/r26b
cargo ndk -t arm64-v8a build --release

# Le binaire sera généré dans :
# target/aarch64-linux-android/release/libpocket_tts_mobile.so
```

### Build pour test local (x86_64)

```bash
cargo build --release --features test-bin
./target/release/pocket-tts-mobile-test --text "Bonjour le monde" --output test.wav
```

---

## 📦 Structure du projet

```
pocket-tts-mobile/
├── Cargo.toml                 # Dépendances et configuration
├── LICENSE                    # Licence MIT + attributions
├── README.md                  # Ce fichier
├── .gitignore
├── src/
│   ├── lib.rs                 # Point d'entrée de la lib
│   ├── ffi.rs                 # ABI C (ptt_*) pour compatibilité Unifia
│   ├── config.rs              # Copie de kyutai-labs/pocket-tts/rust/src/config.rs
│   ├── text.rs                # Copie de text.rs (front-end FR)
│   ├── model.rs               # Copie de model.rs (génération)
│   ├── tokenizer.rs           # Copie de tokenizer.rs
│   ├── weights.rs             # Copie de weights.rs
│   ├── rng.rs                 # Copie de rng.rs
│   └── main.rs                # Binaire de test (feature test-bin)
├── android/
│   ├── build.sh               # Script de build Android
│   └── jniLibs/
│       ├── arm64-v8a/         # libpocket_tts_mobile.so pour ARM64
│       └── x86_64/            # libpocket_tts_mobile.so pour x86_64
└── tests/
    └── fr_benchmark.json       # Jeu de test français
```

---

## 🔌 ABI C (Compatibilité Unifia)

Ce projet expose une **ABI C compatible** avec l'actuel `libpocket_tts.so` (C++) d'Unifia :

| Fonction | Description |
|----------|-------------|
| `ptt_create` | Initialise une nouvelle session TTS. |
| `ptt_stream_start` | Démarre la génération audio (streaming). |
| `ptt_stream_read` | Lit un chunk audio (f32, 24 kHz mono). |
| `ptt_stream_end` | Termine la génération et libère les ressources. |
| `ptt_free_audio` | Libère un buffer audio alloué par `ptt_stream_read`. |

### Exemple d'utilisation (C)

```c
#include "pocket_tts.h"

int main() {
    void* engine = ptt_create("/chemin/vers/pack/fr-24l-q8");
    
    const char* text = "Bonjour le monde";
    ptt_stream_start(engine, text, 1.0, 0);
    
    float buffer[1024];
    size_t n;
    while ((n = ptt_stream_read(engine, buffer, 1024)) > 0) {
        // Traiter le chunk audio (n samples f32, 24 kHz mono)
    }
    
    ptt_stream_end(engine);
    ptt_free(engine);
    return 0;
}
```

---

## 📂 Packs modèles

Les packs modèles doivent contenir :

```
voice-models/pocket/fr-24l-q8/
├── config.json          # Config générée depuis french_24l.yaml
├── model.q8.gguf        # Poids quantifiés (q8)
├── tokenizer.json       # Tokenizer (JSON)
└── embeddings/
    └── estelle.safetensors  # Embedding de la voix
```

### Génération de `config.json`

Utiliser le script `scripts/generate_config.py` pour convertir un YAML Kyutai en `config.json` :

```bash
python scripts/generate_config.py \
  --yaml pocket_tts/config/french_24l.yaml \
  --output voice-models/pocket/fr-24l-q8/config.json
```

### Conversion en GGUF (q8)

Utiliser l'outil `ptts` pour convertir un `model.safetensors` en `model.q8.gguf` :

```bash
ptts quantize --input model.safetensors --output model.q8.gguf --quant q8
```

---

## 🔍 Benchmarks

### Modèles testés

| Modèle | Taille | Précision | RTF (Mi 10 Pro) | TTFA (ms) | RSS (Mo) | WER |
|--------|--------|-----------|------------------|-----------|----------|-----|
| 24L int8 (ONNX) | ~392 Mo | int8 | 1.96–3.19 | ~300 | ~450 | 0.048 |
| 24L q8 (GGUF) | ~250–300 Mo | q8 | ? | ? | ? | ? |
| 24L fp32 | ~900 Mo | fp32 | ? | ? | ? | ? |
| 6L fp32 | ~226 Mo | fp32 | ? | ? | ? | ? |

> **Cible** : 24L q8 avec RTF ≥ 2.0 et WER ≤ 0.05.

---

## 🤝 Contribution

1. **Fork** ce dépôt.
2. **Crée une branche** (`git checkout -b feature/ma-fonctionnalité`).
3. **Commit** tes changements (`git commit -m "feat: description"`).
4. **Push** (`git push origin feature/ma-fonctionnalité`).
5. **Ouvre une PR**.

### Upstream

Ce projet est conçu pour être **proposé en amont** :
- À [xn-ptts](https://github.com/gradium-ai/xn-ptts) : pour exposer `frames_after_eos` dans le FFI.
- À [pocket-tts](https://github.com/kyutai-labs/pocket-tts) : pour ajouter un support Android officiel.

---

## 📜 Licence

- **Code** : [MIT License](LICENSE) (compatible avec les licences upstream MIT/Apache-2.0).
- **Poids** : [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) (Kyutai Labs).

---

## 📞 Contact

- **Propriétaire** : Erwan Barat (`@erwanbarat`)
- **Repo** : [Rwanbt/pocket-tts-mobile](https://github.com/Rwanbt/pocket-tts-mobile)
- **Issues** : [Rwanbt/unifia#](https://github.com/Rwanbt/unifia/issues) (Épic : Portage Rust Pocket TTS)
