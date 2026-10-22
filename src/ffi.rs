//! FFI (Foreign Function Interface) pour Pocket TTS Mobile
//!
//! Ce module expose une **ABI C compatible** avec l'actuel `libpocket_tts.so` (C++)
//! utilisé par Unifia.
//!
//! # Statut
//!
//! **Phase 0 - Work in Progress**
//!
//! Les fonctions sont implémentées comme des stubs pour permettre la compilation.
//! L'implémentation complète sera ajoutée dans les phases suivantes.
//!
//! # ABI Exposée
//!
//! | Fonction | Description | Statut |
//! |----------|-------------|--------|
//! | `ptt_create` | Initialise une nouvelle session TTS | ✅ Stub |
//! | `ptt_free` | Libère une session TTS | ✅ Implémenté |
//! | `ptt_stream_start` | Démarre la génération audio | ✅ Stub |
//! | `ptt_stream_read` | Lit un chunk audio | ✅ Stub |
//! | `ptt_stream_end` | Termine la génération | ✅ Stub |
//! | `ptt_free_audio` | Libère un buffer audio | ✅ Implémenté |

use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;
use std::sync::{Arc, Mutex};

use tracing::error;

use crate::engine::TtsEngine;

/// Opaque handle pour une session TTS (compatible avec l'ABI C++ existante)
pub struct PttHandle {
    engine: Arc<Mutex<TtsEngine>>,
}

// ============================================================================
// Fonctions C exposées (ABI)
// ============================================================================

/// Initialise une nouvelle session TTS avec un pack modèle.
///
/// # Arguments
/// - `pack_path` : Chemin vers le dossier du pack
///
/// # Retour
/// - Pointeur vers une nouvelle session TTS, ou NULL en cas d'erreur.
///
/// # Statut
/// - Stub: retourne toujours NULL pour l'instant
#[no_mangle]
pub extern "C" fn ptt_create(pack_path: *const c_char) -> *mut PttHandle {
    if pack_path.is_null() {
        error!("ptt_create: pack_path is NULL");
        return ptr::null_mut();
    }

    // Convertir le C string en Rust string
    let pack_path_cstr = unsafe { CStr::from_ptr(pack_path) };
    let pack_path = pack_path_cstr.to_str().unwrap_or_default();

    match TtsEngine::new(pack_path) {
        Ok(engine) => {
            let handle = Box::new(PttHandle {
                engine: Arc::new(Mutex::new(engine)),
            });
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
/// - `handle` : Pointeur vers une session TTS
#[no_mangle]
pub extern "C" fn ptt_free(handle: *mut PttHandle) {
    if handle.is_null() {
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
/// - `temperature` : Température pour la génération
/// - `voice_id` : ID de la voix
///
/// # Retour
/// - 0 en cas de succès, -1 en cas d'erreur
///
/// # Statut
/// - Stub: retourne toujours 0 pour l'instant
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

    // Convertir le C string en Rust string
    let text_cstr = unsafe { CStr::from_ptr(text) };
    let text = text_cstr.to_str().unwrap_or_default();

    let engine = unsafe { &*handle };
    let mut engine_guard = engine.engine.lock().unwrap();

    match engine_guard.start_stream(text, temperature, voice_id) {
        Ok(_) => 0,
        Err(e) => {
            error!("ptt_stream_start failed: {}", e);
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
/// # Statut
/// - Stub: retourne toujours 0 pour l'instant
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
    if buffer_size == 0 {
        error!("ptt_stream_read: buffer_size is 0");
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
///
/// # Statut
/// - Stub: retourne toujours 0 pour l'instant
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
///
/// # Note
/// Dans cette implémentation, le buffer est fourni par l'appelant,
/// donc cette fonction ne fait rien.
#[no_mangle]
pub extern "C" fn ptt_free_audio(_buffer: *mut f32) {
    // No-op: le buffer est fourni par l'appelant
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
