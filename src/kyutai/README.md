# Adaptateur Kyutai (code repris)

Ce dossier contient du code repris de
[kyutai-labs/pocket-tts](https://github.com/kyutai-labs/pocket-tts) (branche `main`,
dossier `rust/src/`), licence MIT (voir `../vendor/kyutai-configs/LICENSE`).

| Fichier ici            | Fichier amont                | Modifications                                     |
| ---------------------- | ---------------------------- | ------------------------------------------------- |
| `config.rs`            | `rust/src/config.rs`         | `crate::defaults` / `crate::download` → `crate::kyutai::*` ; `log` → `tracing` |
| `defaults.rs`          | `rust/src/defaults.rs`       | aucune (les tests upstream ont été retirés)       |
| `download.rs`          | `rust/src/download.rs`       | **réduit aux fichiers locaux** : pas de téléchargement à l'exécution |
| `text.rs`              | `rust/src/text.rs`           | **amputé** : seuls `TextOptions` et `prepare_text_prompt` ; le découpage en phrases de l'original est fait par `ptts` (`max_tokens_per_chunk`) |

## Pourquoi cet adaptateur

Les checkpoints publiés par Kyutai (`kyutai/pocket-tts-without-voice-cloning`) sont
décrits par des YAML, alors que `ptts` attend un `config.json` suivant le schéma
serde de `ptts::tts_model::TTSConfig`. `Config::to_ptts` fait la traduction ;
`src/bin/make_config.rs` s'en sert pour produire le `config.json` d'un pack.

## Deux écarts avec `ptts::loader::remap_key`

`ptts::loader::remap_key` (rev `7efa269`) ne fait pas ce que fait
`kyutai-labs/pocket-tts/rust/src/weights.rs` :

1. Kyutai expose le tenseur `flow_lm.bos_before_voice` ; `ptts` attend
   `flow_lm.condition_provider.conditioners.speaker_wavs.sink` (le même tenseur, au
   nom du « sink » de la condition `speaker_wavs`). `ptts` échoue donc au chargement
   avec `tensor 'flow_lm.condition_provider.conditioners.speaker_wavs.sink' not found`.
   `scripts/prepare_pack.py` renomme la clé dans le fichier de poids, hors ligne.
2. Kyutai donne aussi au checkpoint son propre `remap_key` pour les fichiers de
   poids séparés ; le pack mobile n'utilise que le bundle unique, où ce cas ne se pose pas.
