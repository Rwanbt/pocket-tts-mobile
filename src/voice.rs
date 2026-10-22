//! Résolution de la voix dans un pack.
//!
//! Les règles sont celles de l'appelant Unifia
//! (`packages/mobile/src-tauri/src/voice/pocket_tts.rs`), appliquées côté natif
//! pour que la cdylib se comporte pareil quelle que soit la version déployée :
//!
//! 1. la voix demandée, si le pack la contient ;
//! 2. sinon, la voix par défaut du pack ;
//! 3. sinon, la première voix du pack (triée) ;
//! 4. sinon, erreur explicite.
//!
//! La dernière étape n'est jamais un hasard : le pack peut proposer plusieurs
//! locuteurs, et seule la voix demandée par l'utilisateur doit décider. Le
//! tri rend le choix déterministe d'un appel à l'autre.
//!
//! # Format des voix
//!
//! Le pack ONNX utilise des prompts audio `*.wav` ; le pack Rust utilise des
//! embeddings `*.safetensors`. Les deux formats sont acceptés : le filtre
//! porte sur l'extension, pas sur le contenu.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

/// Extension des prompts audio (pack ONNX).
pub const WAV: &str = "wav";
/// Extension des embeddings de voix (pack Rust).
pub const SAFETENSORS: &str = "safetensors";

/// Voix par défaut du pack, si elle est présente.
pub const DEFAULT_VOICE: &str = "voice.wav";

/// Les voix d'un pack, triées, comme noms de fichiers nus.
///
/// Les entrées qui ne sont pas des fichiers, ou dont l'extension n'est pas
/// connue, sont ignorées : un pack peut contenir des fichiers annexes.
pub fn list(voices: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(voices) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let ext = path.extension()?.to_str()?;
            if ext != WAV && ext != SAFETENSORS {
                return None;
            }
            path.file_name()?.to_str().map(str::to_owned)
        })
        .collect();
    names.sort();
    names
}

/// Résout la voix demandée contre ce que le pack contient.
///
/// Retourne le nom de fichier retenu, ou une erreur explicite si le pack
/// ne contient aucune voix.
pub fn resolve(voices: &Path, requested: Option<&str>) -> Result<String> {
    let available = list(voices);
    let wanted = requested
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(DEFAULT_VOICE);
    if available.iter().any(|name| name == wanted) {
        return Ok(wanted.to_owned());
    }
    if available.iter().any(|name| name == DEFAULT_VOICE) {
        return Ok(DEFAULT_VOICE.to_owned());
    }
    if let Some(first) = available.first() {
        return Ok(first.clone());
    }
    Err(anyhow!(
        "no voice in {}: the pack ships no {} or {} file",
        voices.display(),
        WAV,
        SAFETENSORS
    ))
}

/// Résout la voix et renvoie son chemin complet.
pub fn path(voices: &Path, requested: Option<&str>) -> Result<PathBuf> {
    let name = resolve(voices, requested)?;
    Ok(voices.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Crée un dossier de voix avec les fichiers donnés.
    fn pack(tag: &str, names: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ptt-voice-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create voices dir");
        for name in names {
            std::fs::write(dir.join(name), b"x").expect("write voice");
        }
        dir
    }

    #[test]
    fn the_requested_voice_wins_when_the_pack_has_it() {
        let dir = pack("pick", &["voice.wav", "voice-m.wav"]);
        assert_eq!(
            resolve(&dir, Some("voice-m.wav")).expect("resolve"),
            "voice-m.wav"
        );
        assert_eq!(
            resolve(&dir, Some("voice.wav")).expect("resolve"),
            "voice.wav"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_voice_falls_back_to_the_default() {
        let dir = pack("fallback", &["voice.wav", "voice-m.wav"]);
        assert_eq!(
            resolve(&dir, Some("absent.wav")).expect("resolve"),
            "voice.wav"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stale_preference_does_not_silence_the_language() {
        // Une préférence enregistrée avant le renommage du pack nomme un
        // fichier qui n'existe plus. Refuser ici emporterait toute la langue.
        let dir = pack("stale", &["voice-f.wav", "voice-m.wav"]);
        assert_eq!(
            resolve(&dir, Some("voice.wav")).expect("resolve"),
            "voice-f.wav",
            "sans voix par défaut, la première voix triée est retenue"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_fallback_is_deterministic() {
        let dir = pack("deterministic", &["zeta.wav", "alpha.wav"]);
        let first = resolve(&dir, Some("absent.wav")).expect("resolve");
        let second = resolve(&dir, Some("absent.wav")).expect("resolve");
        assert_eq!(first, "alpha.wav");
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_request_uses_the_default() {
        let dir = pack("empty", &["voice.wav"]);
        assert_eq!(resolve(&dir, Some("   ")).expect("resolve"), "voice.wav");
        assert_eq!(resolve(&dir, None).expect("resolve"), "voice.wav");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pack_without_any_voice_is_an_explicit_error() {
        let dir = pack("novoice", &[]);
        let err = resolve(&dir, Some("voice.wav")).expect_err("must fail");
        assert!(err.to_string().contains("no voice in"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_directory_is_an_explicit_error() {
        let err = resolve(Path::new("nowhere/at/all"), None).expect_err("must fail");
        assert!(err.to_string().contains("no voice in"), "{err}");
    }

    #[test]
    fn embeddings_and_prompts_are_both_listed() {
        let dir = pack("both", &["estelle.safetensors", "voice.wav", "notes.txt"]);
        assert_eq!(
            list(&dir),
            vec!["estelle.safetensors".to_owned(), "voice.wav".to_owned()]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_resolved_path_points_at_the_file() {
        let dir = pack("path", &["estelle.safetensors"]);
        let path = path(&dir, Some("estelle.safetensors")).expect("path");
        assert!(path.is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
