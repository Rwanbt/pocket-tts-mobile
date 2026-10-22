//! Résolution des chemins de config Kyutai.
//!
//! Repris de <https://github.com/kyutai-labs/pocket-tts> (`rust/src/download.rs`), MIT,
//! mais **réduit aux fichiers locaux** : le pack mobile est préparé hors ligne, donc
//! aucun téléchargement n'est fait à l'exécution (ni `hf://`, ni `http(s)://`).
//!
//! Voir `vendor/kyutai-configs/LICENSE`.

use anyhow::{Result, bail};
use std::path::PathBuf;

/// Renvoie le chemin local du fichier, ou une erreur si le chemin est distant.
pub fn download_if_necessary(file_path: &str) -> Result<PathBuf> {
    if file_path.starts_with("http://")
        || file_path.starts_with("https://")
        || file_path.starts_with("hf://")
    {
        bail!(
            "this build does not download: {file_path:?} must be prepared offline into the model pack"
        );
    }
    Ok(PathBuf::from(file_path))
}
