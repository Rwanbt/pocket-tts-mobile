//! Adaptateur Kyutai : traduction des configs YAML de `pocket_tts` vers le schéma
//! `config.json` de `ptts`.
//!
//! Code repris de <https://github.com/kyutai-labs/pocket-tts> (`rust/src/`), MIT.
//! Voir `vendor/kyutai-configs/LICENSE` pour la licence d'origine.
//!
//! Trois adaptations par rapport à l'original :
//!
//! - les chemins `crate::defaults` / `crate::download` deviennent `crate::kyutai::*` ;
//! - `download.rs` ne gère que les fichiers locaux : le pack mobile est préparé
//!   hors ligne, aucun téléchargement n'est fait à l'exécution ;
//! - `text.rs` est amputé du découpage en phrases, que `ptts` fait déjà
//!   (`max_tokens_per_chunk`).

pub mod config;
pub mod defaults;
pub mod download;
pub mod text;

/// Le YAML embarqué d'une langue, par son nom (`french_24l`, `english`, ...).
///
/// Les configs viennent de `vendor/kyutai-configs`, embarquées par `build.rs`.
pub fn config_yaml(name: &str) -> Option<&'static str> {
    config::CONFIGS.iter().find(|(n, _)| *n == name).map(|(_, yaml)| *yaml)
}

/// Les noms des langues disponibles.
pub fn available_configs() -> Vec<&'static str> {
    config::CONFIGS.iter().map(|(name, _)| *name).collect()
}
