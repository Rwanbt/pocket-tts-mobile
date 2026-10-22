//! FFI (Foreign Function Interface) — Pocket TTS Mobile.
//!
//! Implémente l'ABI C `ptt_*` **exactement** telle que l'appelle
//! `packages/mobile/src-tauri/src/voice/pocket_tts.rs` dans Unifia.
//! Le contrat complet est dans `docs/ABI-CONTRACT.md` ; toute divergence ici
//! casse l'interopérabilité (la lib C++ d'origine n'est plus utilisée).
//!
//! # Symboles exposés
//!
//! | Symbole | Rôle |
//! |---------|------|
//! | `ptt_create` | Crée un moteur depuis un pack. NULL en cas d'échec. |
//! | `ptt_destroy` | Libère le moteur. Exactement une fois. |
//! | `ptt_warmup` | Préchauffe le moteur, renvoie la durée en ms. |
//! | `ptt_set_text_options` | Applique le front-end texte. 0 = accepté. |
//! | `ptt_stream_start` | Démarre une synthèse, renvoie un contexte. |
//! | `ptt_stream_read` | Lecture bloquante, **alloue** le tampon. 1 = chunk. |
//! | `ptt_stream_end` | Arrête le stream, libère le contexte. |
//! | `ptt_free_audio` | Libère un tampon de `ptt_stream_read`. |
//!
//! # Safety
//!
//! - L'ABI C++ d'origine n'a pas de canal d'erreur : un échec se manifeste par
//!   un pointeur nul ou un retour ≠ 1. Chaque échec est donc **logué** côté
//!   natif, sinon il est invisible (la console du WebView ne rejoint pas logcat).
//! - `ptt_stream_read` renvoie un tampon alloué par Rust : l'appelant doit le
//!   libérer avec `ptt_free_audio`, et lui seul.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::Mutex;

use tracing::{error, info};

use crate::engine::{Engine, Stream};

/// Moteur TTS vu par l'ABI C.
pub struct Handle {
    engine: Mutex<Engine>,
}

/// Contexte de stream vu par l'ABI C.
pub struct Ctx {
    stream: Stream,
}

/// Traduit une chaîne C en `&str`.
///
/// # Safety
/// `ptr` doit pointer vers une chaîne C terminée par NUL, valide pendant l'appel.
unsafe fn cstr<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}

/// Crée un moteur à partir d'un pack.
///
/// Renvoie `NULL` en cas d'échec ; l'erreur est loguée.
///
/// # Safety
/// Les pointeurs doivent être des chaînes C vivantes pendant l'appel.
#[no_mangle]
pub unsafe extern "C" fn ptt_create(
    models_dir: *const c_char,
    voices_dir: *const c_char,
    tokenizer: *const c_char,
    precision: *const c_char,
    temperature: f32,
    lsd_steps: c_int,
    num_threads: c_int,
) -> *mut c_void {
    let (Some(models), Some(voices), Some(tok)) =
        (unsafe { (cstr(models_dir), cstr(voices_dir), cstr(tokenizer)) })
    else {
        error!("ptt_create: NULL path argument");
        return ptr::null_mut();
    };
    let prec = unsafe { cstr(precision) }.unwrap_or("f32");

    match Engine::load(
        models,
        voices,
        tok,
        prec,
        temperature,
        lsd_steps,
        num_threads,
    ) {
        Ok(engine) => {
            info!(
                "[voice] Pocket Rust engine loaded: models={models} voices={voices} \
                 tokenizer={tok} precision={prec} temperature={temperature} \
                 lsd_steps={lsd_steps} num_threads={num_threads}"
            );
            Box::into_raw(Box::new(Handle {
                engine: Mutex::new(engine),
            })) as *mut c_void
        }
        Err(err) => {
            error!("ptt_create failed: {err}");
            ptr::null_mut()
        }
    }
}

/// Libère un moteur.
///
/// # Safety
/// `engine` doit venir de `ptt_create` et n'être libéré qu'une fois, après
/// la fin de tous ses streams.
#[no_mangle]
pub unsafe extern "C" fn ptt_destroy(engine: *mut c_void) {
    if engine.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(engine as *mut Handle));
    }
}

/// Préchauffe le moteur et renvoie la durée en ms.
///
/// # Safety
/// `engine` doit venir de `ptt_create` et être vivant.
#[no_mangle]
pub unsafe extern "C" fn ptt_warmup(engine: *mut c_void) -> f64 {
    if engine.is_null() {
        return 0.0;
    }
    let handle = unsafe { &*(engine as *const Handle) };
    let mut guard = match handle.engine.lock() {
        Ok(guard) => guard,
        Err(err) => {
            error!("ptt_warmup: engine poisoned: {err}");
            return 0.0;
        }
    };
    match guard.warmup() {
        Ok(ms) => ms,
        Err(err) => {
            error!("ptt_warmup failed: {err}");
            0.0
        }
    }
}

/// Applique les options du front-end texte.
///
/// Renvoie 0 si les options sont acceptées, non-zéro sinon. Dans ce dernier
/// cas l'appelant détruit le moteur.
///
/// # Safety
/// `engine` doit venir de `ptt_create`.
#[no_mangle]
pub unsafe extern "C" fn ptt_set_text_options(
    engine: *mut c_void,
    eos_extra_frames: c_int,
    pad_short_inputs: c_int,
    remove_semicolons: c_int,
    max_chunk_tokens: c_int,
) -> c_int {
    if engine.is_null() {
        return 1;
    }
    let handle = unsafe { &*(engine as *const Handle) };
    let mut guard = match handle.engine.lock() {
        Ok(guard) => guard,
        Err(err) => {
            error!("ptt_set_text_options: engine poisoned: {err}");
            return 1;
        }
    };
    match guard.set_text_options(
        eos_extra_frames,
        pad_short_inputs != 0,
        remove_semicolons != 0,
        max_chunk_tokens,
    ) {
        Ok(()) => 0,
        Err(err) => {
            error!("ptt_set_text_options rejected: {err}");
            1
        }
    }
}

/// Démarre une synthèse et renvoie un contexte de stream.
///
/// Renvoie `NULL` en cas d'échec.
///
/// # Safety
/// `engine` doit venir de `ptt_create` ; `text` et `voice` doivent être des
/// chaînes C vivantes pendant l'appel.
#[no_mangle]
pub unsafe extern "C" fn ptt_stream_start(
    engine: *mut c_void,
    text: *const c_char,
    voice: *const c_char,
) -> *mut c_void {
    if engine.is_null() {
        error!("ptt_stream_start: NULL engine");
        return ptr::null_mut();
    }
    let (Some(txt), Some(voc)) = (unsafe { (cstr(text), cstr(voice)) }) else {
        error!("ptt_stream_start: NULL text or voice");
        return ptr::null_mut();
    };
    let handle = unsafe { &*(engine as *const Handle) };
    let mut guard = match handle.engine.lock() {
        Ok(guard) => guard,
        Err(err) => {
            error!("ptt_stream_start: engine poisoned: {err}");
            return ptr::null_mut();
        }
    };
    match guard.start(txt, voc) {
        Ok(stream) => Box::into_raw(Box::new(Ctx { stream })) as *mut c_void,
        Err(err) => {
            error!("ptt_stream_start failed: {err}");
            ptr::null_mut()
        }
    }
}

/// Lecture bloquante du prochain chunk.
///
/// Renvoie 1 et alloue `*out_samples` (24 kHz mono f32) quand un chunk est
/// disponible ; `*out_length` reçoit le nombre de samples. Toute autre valeur
/// signale la fin du stream ou une erreur, et `*out_samples` reste nul.
///
/// Le tampon appartient à l'appelant, qui doit le libérer avec `ptt_free_audio`.
///
/// # Safety
/// `stream` doit venir de `ptt_stream_start` et être vivant.
#[no_mangle]
pub unsafe extern "C" fn ptt_stream_read(
    stream: *mut c_void,
    out_samples: *mut *mut f32,
    out_length: *mut c_int,
) -> c_int {
    if stream.is_null() || out_samples.is_null() || out_length.is_null() {
        error!("ptt_stream_read: NULL argument");
        return 0;
    }
    unsafe {
        *out_samples = ptr::null_mut();
        *out_length = 0;
    }
    let ctx = unsafe { &mut *(stream as *mut Ctx) };
    match ctx.stream.next() {
        Some(samples) if !samples.is_empty() => {
            let len = c_int::try_from(samples.len()).unwrap_or(c_int::MAX);
            let ptr = alloc_audio(&samples);
            unsafe {
                *out_samples = ptr;
                *out_length = len;
            }
            1
        }
        Some(_) => 0,
        None => 0,
    }
}

/// Arrête un stream et libère son contexte.
///
/// # Safety
/// `stream` doit venir de `ptt_stream_start` et ne pas avoir déjà été libéré.
#[no_mangle]
pub unsafe extern "C" fn ptt_stream_end(stream: *mut c_void) {
    if stream.is_null() {
        return;
    }
    let mut ctx = unsafe { Box::from_raw(stream as *mut Ctx) };
    ctx.stream.stop();
}

/// Libère un tampon renvoyé par `ptt_stream_read`.
///
/// # Safety
/// `samples` doit provenir de `ptt_stream_read` et n'avoir pas déjà été libéré.
#[no_mangle]
pub unsafe extern "C" fn ptt_free_audio(samples: *mut f32) {
    if samples.is_null() {
        return;
    }
    unsafe { free_audio(samples) };
}

/// Alloue un tampon f32 précédé d'un en-tête contenant sa longueur.
///
/// L'ABI C d'origine expose `ptt_free_audio(float*)` sans longueur : sans
/// en-tête, la libération serait un double free ou une lecture hors bornes.
/// L'en-tête tient en un `u32`, ce qui préserve l'alignement 4 octets des
/// `f32`.
fn alloc_audio(samples: &[f32]) -> *mut f32 {
    let len = samples.len();
    let mut words: Vec<u32> = vec![0; len + 1];
    words[0] = len as u32;
    for (slot, sample) in words[1..].iter_mut().zip(samples) {
        *slot = sample.to_bits();
    }
    let base = words.as_mut_ptr();
    std::mem::forget(words);
    // SAFETY: `base` pointe sur len + 1 mots alloués, l'offset 1 garde
    // l'alignement de 4 octets requis par f32.
    unsafe { base.add(1) as *mut f32 }
}

/// Libère un tampon produit par [`alloc_audio`].
///
/// # Safety
/// `ptr` doit provenir de `alloc_audio` et n'avoir pas déjà été libéré.
unsafe fn free_audio(ptr: *mut f32) {
    let base = (ptr as *mut u32).sub(1);
    let len = *base as usize;
    // La reconstruction doit retrouver exactement la longueur ET la capacité
    // d'origine, sinon la libération est un comportement indéfini.
    drop(Vec::from_raw_parts(base, len + 1, len + 1));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_arguments_are_rejected() {
        // SAFETY: les pointeurs sont volontairement invalides, la fonction
        // doit les refuser sans les dereferencer.
        unsafe {
            assert!(ptt_create(
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                0.0,
                0,
                0
            )
            .is_null());
            assert!(ptt_stream_start(ptr::null_mut(), ptr::null(), ptr::null()).is_null());
            assert_eq!(
                ptt_stream_read(ptr::null_mut(), ptr::null_mut(), ptr::null_mut()),
                0
            );
            assert_eq!(ptt_warmup(ptr::null_mut()), 0.0);
            assert_eq!(ptt_set_text_options(ptr::null_mut(), 0, 0, 0, 0), 1);
        }
    }

    #[test]
    fn destroy_and_end_tolerate_null() {
        // SAFETY: NULL est accepte par ces trois fonctions.
        unsafe {
            ptt_destroy(ptr::null_mut());
            ptt_stream_end(ptr::null_mut());
            ptt_free_audio(ptr::null_mut());
        }
    }

    #[test]
    fn free_audio_accepts_a_null_pointer() {
        // SAFETY: NULL est accepte.
        unsafe { ptt_free_audio(ptr::null_mut()) };
    }

    #[test]
    fn audio_round_trips_through_the_allocator() {
        // L'en-tête de longueur est la seule façon de libérer correctement un
        // tampon dont l'ABI C ne transmet pas la taille.
        let samples = vec![0.5f32, -0.25, 1.0, -1.0, 0.0];
        let ptr = alloc_audio(&samples);
        assert!(!ptr.is_null());
        // SAFETY: `ptr` vient de `alloc_audio` et n'est pas encore libéré.
        let len = unsafe { *(ptr as *const u32).sub(1) } as usize;
        assert_eq!(len, samples.len());
        // SAFETY: `ptr` porte `len` samples valides, alloués par `alloc_audio`.
        let read = unsafe { std::slice::from_raw_parts(ptr, len) };
        assert_eq!(read, &samples[..]);
        // SAFETY: le tampon est libéré une seule fois.
        unsafe { free_audio(ptr) };
    }

    #[test]
    fn the_allocator_survives_every_size() {
        // Balayage exhaustif : un décalage d'un octet dans l'en-tête se
        // verrait sur une seule taille, pas sur toutes.
        for len in 0..=4096usize {
            let samples: Vec<f32> = (0..len).map(|i| i as f32 * 0.5 - 2048.0).collect();
            let ptr = alloc_audio(&samples);
            assert!(!ptr.is_null(), "allocation refusée pour {len} samples");
            // SAFETY: `ptr` vient de `alloc_audio` et n'est pas encore libéré.
            let got = unsafe { *(ptr as *const u32).sub(1) } as usize;
            assert_eq!(got, len, "longueur erronée pour {len} samples");
            // L'alignement f32 (4 octets) doit être préservé par l'en-tête.
            assert_eq!(
                (ptr as usize) % std::mem::align_of::<f32>(),
                0,
                "alignement cassé pour {len} samples"
            );
            // SAFETY: `ptr` porte `len` samples valides, alloués par `alloc_audio`.
            let read = unsafe { std::slice::from_raw_parts(ptr, len) };
            assert_eq!(read, &samples[..], "corruption pour {len} samples");
            // SAFETY: le tampon est libéré une seule fois.
            unsafe { free_audio(ptr) };
        }
    }

    #[test]
    fn an_empty_chunk_allocates_a_usable_buffer() {
        let ptr = alloc_audio(&[]);
        assert!(!ptr.is_null());
        // SAFETY: `ptr` vient de `alloc_audio` ; l'en-tête indique 0 sample.
        let got = unsafe { *(ptr as *const u32).sub(1) } as usize;
        assert_eq!(got, 0);
        // SAFETY: le tampon est libéré une seule fois.
        unsafe { free_audio(ptr) };
    }
}
