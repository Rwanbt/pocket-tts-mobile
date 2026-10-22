//! Adaptateur Kyutai : le runtime officiel de Pocket TTS, adapte pour Android.
//!
//! Code repris de <https://github.com/kyutai-labs/pocket-tts> (`rust/src/`), MIT.
//! Voir `vendor/kyutai-configs/LICENSE` pour la licence d'origine.
//!
//! # Ce que contient ce module
//!
//! | Fichier | Role |
//! |---------|------|
//! | `config.rs` | Les configs YAML de `pocket_tts`, et leur traduction vers le schema `config.json` de `ptts` |
//! | `defaults.rs` | Les valeurs par defaut, derivees du nom de la langue |
//! | `download.rs` | La resolution des chemins de poids |
//! | `text.rs` | Le front-end texte : decoupage en phrases, options de ponctuation |
//! | `tokenizer.rs` | La tokenisation |
//! | `weights.rs` | Le `remap_key` de Kyutai et la lecture des etats de voix |
//! | `rng.rs` | Le bruit de la generation |
//! | `audio.rs` | La lecture des WAV (prompts de voix) |
//! | `model.rs` | L'orchestrateur : etat du flow LM, voix, streaming |
//!
//! # Pourquoi le runtime complet, et pas `ptts::synth`
//!
//! `ptts::synth::Synth` est l'API de haut niveau du moteur : elle refait le travail
//! d'orchestration de Kyutai, et en mieux. Les voix publiees par Kyutai ne sont pas
//! des embeddings mais des etats de cache KV, que `ptts::synth` rejette silencieusement :
//! le modele se charge, `voices` est vide, et la generation se fait sans condition
//! vocale. C'est le runtime de Kyutai qui sait les lire, grace a `weights.rs`.
//!
//! Le moteur, lui, reste partage : `model.rs` utilise `ptts::tts_model::TTSModel`, le
//! meme moteur que `ptts::synth` utilise en dessous. Il n'y a donc aucune duplication
//! de moteur, seulement le choix de la bonne couche.
//!
//! # Adaptations par rapport a l'original
//!
//! - les chemins `crate::<module>` deviennent `crate::kyutai::<module>` ;
//! - `download.rs` ne gere que les fichiers locaux : le pack mobile est prepare hors
//!   ligne, aucun telechargement n'est fait a l'execution.

pub mod audio;
pub mod config;
pub mod defaults;
pub mod download;
pub mod model;
pub mod rng;
pub mod text;
pub mod tokenizer;
pub mod weights;

/// Le YAML embarque d'une langue, par son nom (`french_24l`, `english`, ...).
///
/// Les configs viennent de `vendor/kyutai-configs`, embarquees par `build.rs`.
pub fn config_yaml(name: &str) -> Option<&'static str> {
    config::CONFIGS.iter().find(|(n, _)| *n == name).map(|(_, yaml)| *yaml)
}

/// Les noms des langues disponibles.
pub fn available_configs() -> Vec<&'static str> {
    config::CONFIGS.iter().map(|(name, _)| *name).collect()
}
