//! Pocket TTS Mobile - Runtime Rust pour Android
//!
//! Ce crate expose une ABI C compatible avec l'actuel `libpocket_tts.so` (C++)
//! utilisé par Unifia, en réutilisant l'adaptateur Kyutai et le moteur `ptts` (xn-ptts).
//!
//! # Architecture
//!
//! - `ffi.rs` : ABI C (`ptt_create`, `ptt_stream_start`, etc.) pour compatibilité Unifia.
//! - `config.rs` : Chargement et conversion des configs YAML Kyutai vers `TTSConfig` (ptts).
//! - `text.rs` : Front-end texte (découpage en chunks, gestion des options FR).
//! - `model.rs` : Génération audio (appel à `ptts` avec `frames_after_eos`, etc.).
//! - `tokenizer.rs` : Tokenization (via `tokenizers` ou `sentencepiece`).
//! - `weights.rs` : Chargement des poids (safetensors/GGUF).
//!
//! # Licence
//!
//! Code : MIT
//! Poids : CC BY 4.0 (Kyutai Labs)

#![allow(clippy::needless_doctest_main)]

// Module FFI (ABI C)
pub mod ffi;

// Module local pour l'intégration
pub mod engine;

// Réexporter les types publics pour l'ABI
pub use ffi::{
    ptt_create, ptt_free, ptt_free_audio, ptt_stream_end, ptt_stream_read, ptt_stream_start,
};

/// Version du crate (pour compatibilité ABI)
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Nom du crate
pub const NAME: &str = env!("CARGO_PKG_NAME");
