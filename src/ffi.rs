//! FFI (Foreign Function Interface) pour Pocket TTS Mobile
//!
//! Ce module expose une **ABI C compatible** avec l'actuel `libpocket_tts.so` (C++)
//! utilisé par Unifia. Cela permet de remplacer le moteur C++/ONNX par ce runtime Rust
//! sans modifier le code appelant (Tauri/Unifia).
//!
//! # ABI Exposée
//!
//! | Fonction | Description |
//! |----------|-------------|
//! | `ptt_create` | Initialise une nouvelle session TTS avec un pack modèle. |
//! | `ptt_free` | Libère une session TTS. |
//! | `ptt_stream_start` | Démarre la génération audio (streaming). |
//! | `ptt_stream_read` | Lit un chunk audio (f32, 24 kHz mono). |
//! | `ptt_stream_end` | Termine la génération et libère les ressources. |
//! | `ptt_free_audio` | Libère un buffer audio alloué par `ptt_stream_read`. |
//!
//! # Threading
//!
//! - `ptt_stream_start` lance un **thread dédié** pour la génération.
//! - `ptt_stream_read` est **bloquant** si aucun chunk n'est disponible (avec timeout).
//! - `ptt_stream_end` envoie un signal d'arrêt au thread et joint.
//!
//! # Sécurité
//!
//! - Tous les pointeurs retournés sont **valides** jusqu'à l'appel suivant sur la même session.
//! - Les buffers audio sont **alloués par Rust** et doivent être libérés avec `ptt_free_audio`.
//! - Les erreurs sont **remontées** via des codes de retour (NULL ou taille 0).

use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use tracing::{debug, error, info, warn};

use crate::model::TtsEngine;

/// Opaque handle pour une session TTS (compatible avec l'ABI C++ existante)
pub struct PttHandle {
    engine: Arc<Mutex<TtsEngine>>,
}

/// Contexte de streaming pour une session TTS
pub struct StreamContext {
    handle: *mut PttHandle,
    // Autres champs pour gérer le streaming
}

// ============================================================================
// Fonctions C exposées (ABI)
// ============================================================================

/// Initialise une nouvelle session TTS avec un pack modèle.
///
/// # Arguments
/// - `pack_path` : Chemin vers le dossier du pack (ex: `/sdcard/.../voice-models/pocket/fr-24l-q8`)
///
/// # Retour
/// - Pointeur vers une nouvelle session TTS, ou NULL en cas d'erreur.
///
/// # Sécurité
/// - Le pointeur doit être libéré avec `ptt_free`.
#[no_mangle]
pub extern "C" fn ptt_create(pack_path: *const c_char) -> *mut PttHandle {
    if pack_path.is_null() {
        error!("ptt_create: pack_path is NULL");
        return ptr::null_mut();
    }

    let pack_path_str = unsafe { CString::from_raw(pack_path) };
    let pack_path = pack_path_str.to_str().unwrap_or_default();

    match TtsEngine::new(pack_path) {
        Ok(engine) => {
            let handle = Box::new(PttHandle {
                engine: Arc::new(Mutex::new(engine)),
            });
            // Ne pas libérer pack_path_str (le caller doit le faire)
            std::mem::forget(pack_path_str);
            Box::into_raw(handle)
        }
        Err(e) => {
            error!("ptt_create failed: {}", e);
            ptr::null_mut()
        }
    }
}

/// Libère une session TTS.
///
/// # Arguments
/// - `handle` : Pointeur vers une session TTS (retourné par `ptt_create`)
#[no_mangle]
pub extern "C" fn ptt_free(handle: *mut PttHandle) {
    if handle.is_null() {
        warn!("ptt_free: handle is NULL");
        return;
    }
    unsafe {
        let _ = Box::from_raw(handle);
    }
}

/// Démarre la génération audio pour un texte donné.
///
/// # Arguments
/// - `handle` : Pointeur vers une session TTS
/// - `text` : Texte à synthétiser (UTF-8)
/// - `temperature` : Température pour la génération (0.0 = déterministe, >0.0 = aléatoire)
/// - `voice_id` : ID de la voix (0 = voix par défaut)
///
/// # Retour
/// - 0 en cas de succès, -1 en cas d'erreur
#[no_mangle]
pub extern "C" fn ptt_stream_start(
    handle: *mut PttHandle,
    text: *const c_char,
    temperature: f32,
    voice_id: i32,
) -> i32 {
    if handle.is_null() {
        error!("ptt_stream_start: handle is NULL");
        return -1;
    }
    if text.is_null() {
        error!("ptt_stream_start: text is NULL");
        return -1;
    }

    let text_str = unsafe { CString::from_raw(text) };
    let text = text_str.to_str().unwrap_or_default();

    let engine = unsafe { &*handle };
    let mut engine_guard = engine.engine.lock().unwrap();

    match engine_guard.start_stream(text, temperature, voice_id) {
        Ok(_) => {
            // Ne pas libérer text_str (le caller doit le faire)
            std::mem::forget(text_str);
            0
        }
        Err(e) => {
            error!("ptt_stream_start failed: {}", e);
            std::mem::forget(text_str);
            -1
        }
    }
}

/// Lit un chunk audio depuis le stream.
///
/// # Arguments
/// - `handle` : Pointeur vers une session TTS
/// - `buffer` : Buffer pour stocker les samples audio (f32, 24 kHz mono)
/// - `buffer_size` : Taille du buffer (en nombre de samples)
///
/// # Retour
/// - Nombre de samples lus, ou 0 si fin de stream, ou -1 en cas d'erreur
///
/// # Sécurité
/// - Le buffer est **rempli par Rust** et doit être libéré avec `ptt_free_audio` si alloué.
/// - En mode streaming, ce buffer est **toujours alloué par Rust** (même si `buffer` est fourni).
#[no_mangle]
pub extern "C" fn ptt_stream_read(
    handle: *mut PttHandle,
    buffer: *mut f32,
    buffer_size: usize,
) -> i32 {
    if handle.is_null() {
        error!("ptt_stream_read: handle is NULL");
        return -1;
    }
    if buffer.is_null() {
        error!("ptt_stream_read: buffer is NULL");
        return -1;
    }

    let engine = unsafe { &*handle };
    let mut engine_guard = engine.engine.lock().unwrap();

    match engine_guard.read_stream(buffer, buffer_size) {
        Ok(n) => n as i32,
        Err(e) => {
            error!("ptt_stream_read failed: {}", e);
            -1
        }
    }
}

/// Termine la génération audio et libère les ressources du stream.
///
/// # Arguments
/// - `handle` : Pointeur vers une session TTS
///
/// # Retour
/// - 0 en cas de succès, -1 en cas d'erreur
#[no_mangle]
pub extern "C" fn ptt_stream_end(handle: *mut PttHandle) -> i32 {
    if handle.is_null() {
        error!("ptt_stream_end: handle is NULL");
        return -1;
    }

    let engine = unsafe { &*handle };
    let mut engine_guard = engine.engine.lock().unwrap();

    match engine_guard.end_stream() {
        Ok(_) => 0,
        Err(e) => {
            error!("ptt_stream_end failed: {}", e);
            -1
        }
    }
}

/// Libère un buffer audio alloué par `ptt_stream_read`.
///
/// # Arguments
/// - `buffer` : Pointeur vers le buffer à libérer
#[no_mangle]
pub extern "C" fn ptt_free_audio(buffer: *mut f32) {
    if buffer.is_null() {
        warn!("ptt_free_audio: buffer is NULL");
        return;
    }
    unsafe {
        let _ = Vec::from_raw_parts(buffer, 0, 0);
    }
}

// ============================================================================
// Tests unitaires
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ptt_create_null() {
        let handle = ptt_create(ptr::null());
        assert!(handle.is_null());
    }

    #[test]
    fn test_ptt_free_null() {
        ptt_free(ptr::null_mut());
        // Ne doit pas paniquer
    }

    #[test]
    fn test_ptt_stream_start_null() {
        let result = ptt_stream_start(ptr::null_mut(), ptr::null(), 0.0, 0);
        assert_eq!(result, -1);
    }

    #[test]
    fn test_ptt_stream_read_null() {
        let result = ptt_stream_read(ptr::null_mut(), ptr::null_mut(), 0);
        assert_eq!(result, -1);
    }

    #[test]
    fn test_ptt_stream_end_null() {
        let result = ptt_stream_end(ptr::null_mut());
        assert_eq!(result, -1);
    }

    #[test]
    fn test_ptt_free_audio_null() {
        ptt_free_audio(ptr::null_mut());
        // Ne doit pas paniquer
    }
}
