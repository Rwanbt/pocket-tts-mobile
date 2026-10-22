//! Backends de synthèse — Pocket TTS Mobile.
//!
//! Le backend porte l'appel au moteur d'inférence. L'ABI C, les options du
//! front-end texte et la mécanique de streaming vivent dans [`crate::engine`]
//! et [`crate::ffi`] ; ici, seule la génération change selon le moteur.
//!
//! # Implémentations
//!
//! - [`Stub`] : bouchon de la Phase 0/1. Il ne synthétise rien mais expose
//!   l'API complète, ce qui permet d'exercer et de tester l'ABI sans les poids.
//! - `Ptts` :分支 sur le moteur `ptts` (xn-ptts). À brancher dès que son API
//!   de construction de `Synth` est connue (voir issue #873).

use anyhow::Result;

use crate::engine::TextOptions;

/// Erreur renvoyée par un backend qui ne peut pas encore synthétiser.
pub const NOT_IMPLEMENTED: &str =
    "the ptts backend is not wired yet: see Rwanbt/unifia#873 for the API discovery";

/// Un backend de synthèse.
///
/// `Send + Sync` : le thread de génération partage le backend avec le moteur.
pub trait Backend: Send + Sync {
    /// Préchauffe le moteur (chargement des poids hors du chemin critique).
    fn warmup(&self) -> Result<()>;

    /// Synthétise `text` avec la voix `voice` et pousse les chunks dans `sink`.
    ///
    /// Le backend respecte l'annulation : il retourne dès que `sink` refuse
    /// le chunk.
    fn generate(
        &self,
        text: &str,
        voice: &str,
        options: TextOptions,
        temperature: f32,
        sink: &mut dyn FnMut(Vec<f32>) -> Result<()>,
    ) -> Result<()>;
}

/// Bouchon : charge les paramètres mais ne synthétise pas.
pub struct Stub {
    models: String,
    voices: String,
    tokenizer: String,
    precision: String,
    lsd_steps: i32,
    num_threads: i32,
}

impl Stub {
    /// Crée un bouchon qui mémorise la configuration du pack.
    pub fn new(
        models: &str,
        voices: &str,
        tokenizer: &str,
        precision: &str,
        lsd_steps: i32,
        num_threads: i32,
    ) -> Self {
        Self {
            models: models.to_owned(),
            voices: voices.to_owned(),
            tokenizer: tokenizer.to_owned(),
            precision: precision.to_owned(),
            lsd_steps,
            num_threads,
        }
    }

    /// Configuration mémorisée, pour les tests et les logs.
    pub fn config(&self) -> (&str, &str, &str, &str, i32, i32) {
        (
            &self.models,
            &self.voices,
            &self.tokenizer,
            &self.precision,
            self.lsd_steps,
            self.num_threads,
        )
    }
}

impl Backend for Stub {
    fn warmup(&self) -> Result<()> {
        tracing::info!(
            "[voice] Pocket Rust stub warmup: models={} voices={} tokenizer={} precision={} \
             lsd_steps={} num_threads={}",
            self.models,
            self.voices,
            self.tokenizer,
            self.precision,
            self.lsd_steps,
            self.num_threads
        );
        Ok(())
    }

    fn generate(
        &self,
        text: &str,
        voice: &str,
        _options: TextOptions,
        _temperature: f32,
        _sink: &mut dyn FnMut(Vec<f32>) -> Result<()>,
    ) -> Result<()> {
        anyhow::bail!("{NOT_IMPLEMENTED} (text={text:?}, voice={voice:?})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_keeps_the_pack_configuration() {
        let stub = Stub::new("m", "v", "t", "int8", 1, 2);
        assert_eq!(stub.config(), ("m", "v", "t", "int8", 1, 2));
    }

    #[test]
    fn the_stub_refuses_to_synthesise() {
        let stub = Stub::new("m", "v", "t", "int8", 1, 2);
        let mut sink = |_chunk: Vec<f32>| Ok(());
        let err = stub
            .generate(
                "bonjour",
                "voice.wav",
                TextOptions::default(),
                0.5,
                &mut sink,
            )
            .expect_err("stub must refuse");
        assert!(err.to_string().contains("not wired yet"), "{err}");
    }
}
