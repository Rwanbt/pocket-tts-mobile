//! Moteur TTS et streaming — Pocket TTS Mobile.
//!
//! L'ABI C `ptt_*` vit dans [`crate::ffi`]. Ce module porte l'état du moteur
//! (pack chargé, options du front-end texte) et la mécanique de streaming
//! (thread de génération + canal borné + annulation).
//!
//! # Front-end texte
//!
//! Les quatre options exposées par `ptt_set_text_options` reproduisent celles
//! du front-end Kyutai pour le français (`french_24l.yaml`) :
//! `eos_extra_frames = 8`, pas de padding, `;` lu comme `,`, chunks de 50
//! tokens. Ce sont elles qui évitent le bug « mot final avalé » (3 frames
//! après EOS au lieu de 8) : les appliquer ici, et non dans l'appelant,
//! garantit la parité avec le runtime officiel.

use anyhow::{anyhow, Result};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::backend::{Backend, Stub};

/// Options du front-end texte, telles que passées par `ptt_set_text_options`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextOptions {
    /// Frames rendues après le déclenchement de l'EOS ; -1 laisse le runtime
    /// deviner (3 ou 5).
    pub eos_extra_frames: i32,
    /// Préfixe les phrases de moins de cinq mots par huit espaces.
    pub pad_short_inputs: bool,
    /// Lit ';' comme ','.
    pub remove_semicolons: bool,
    /// Regroupe les phrases par paquets d'au plus ce nombre de tokens ;
    /// 0 génère chaque phrase séparément.
    pub max_chunk_tokens: i32,
}

impl Default for TextOptions {
    /// Reprend le comportement amont de PocketTTS.cpp, que les packs
    /// 6 couches utilisent encore.
    fn default() -> Self {
        Self {
            eos_extra_frames: -1,
            pad_short_inputs: true,
            remove_semicolons: false,
            max_chunk_tokens: 0,
        }
    }
}

/// Nombre de samples par chunk transmis au canal.
///
/// C'est aussi la taille de chunk que les backends produisent : elle borne
/// la mémoire du tampon entre le thread de génération et le lecteur. La
/// génération étant plus rapide que la lecture, un canal borné oblige le
/// producteur à attendre au lieu d'accumuler.
pub const CHUNK: usize = 1024;

/// Moteur TTS chargé, prêt à synthétiser.
pub struct Engine {
    /// Le backend est partagé avec le thread de génération : il doit donc
    /// être détenu par un `Arc` plutôt que emprunté depuis `self`.
    backend: Arc<dyn Backend>,
    /// Température de génération, fixée à la création du moteur.
    temperature: f32,
    /// Options du front-end texte, ajustables jusqu'au premier stream.
    options: TextOptions,
    /// Vrai une fois qu'un stream a démarré : les options sont alors figées.
    started: bool,
}

impl Engine {
    /// Charge un pack et prépare le moteur.
    pub fn load(
        models: &str,
        voices: &str,
        tokenizer: &str,
        precision: &str,
        temperature: f32,
        lsd_steps: i32,
        num_threads: i32,
    ) -> Result<Self> {
        // Le pack est validé avant tout : un pack incomplet doit faire
        // échouer `ptt_create` (pointeur nul), pas la première synthèse.
        check_pack(models, voices, tokenizer)?;

        // TODO(phase-1): brancher le moteur `ptts` (xn-ptts) une fois son API
        // de construction de `Synth` connue. D'ici là, le backend est un
        // bouchon qui refuse de synthétiser, pour que le reste de l'ABI soit
        // exercable et testable.
        let backend: Arc<dyn Backend> = Arc::new(Stub::new(
            models,
            voices,
            tokenizer,
            precision,
            lsd_steps,
            num_threads,
        ));
        Ok(Self::with_backend(backend, temperature))
    }

    /// Construit un moteur autour d'un backend fourni.
    ///
    /// Sert aux tests, qui injectent un backend déterministe, et aux phases
    /// suivantes, où le backend `ptts` remplacera le bouchon.
    pub fn with_backend(backend: Arc<dyn Backend>, temperature: f32) -> Self {
        Self {
            backend,
            temperature,
            options: TextOptions::default(),
            started: false,
        }
    }

    /// Options du front-end texte actuellement appliquées.
    pub fn options(&self) -> TextOptions {
        self.options
    }

    /// Préchauffe le moteur et renvoie la durée en ms.
    pub fn warmup(&mut self) -> Result<f64> {
        let start = Instant::now();
        self.backend.warmup()?;
        Ok(start.elapsed().as_secs_f64() * 1000.0)
    }

    /// Applique les options du front-end texte.
    pub fn set_text_options(
        &mut self,
        eos_extra_frames: i32,
        pad_short_inputs: bool,
        remove_semicolons: bool,
        max_chunk_tokens: i32,
    ) -> Result<()> {
        if self.started {
            return Err(anyhow!("text options cannot change after a stream started"));
        }
        if eos_extra_frames < -1 {
            return Err(anyhow!(
                "eos_extra_frames must be -1 or more, got {eos_extra_frames}"
            ));
        }
        if max_chunk_tokens < 0 {
            return Err(anyhow!(
                "max_chunk_tokens must not be negative, got {max_chunk_tokens}"
            ));
        }
        self.options = TextOptions {
            eos_extra_frames,
            pad_short_inputs,
            remove_semicolons,
            max_chunk_tokens,
        };
        Ok(())
    }

    /// Démarre une synthèse et renvoie le contexte de stream.
    pub fn start(&mut self, text: &str, voice: &str) -> Result<Stream> {
        self.started = true;
        let (tx, rx) = sync_channel::<Vec<f32>>(8);
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&stop);
        let options = self.options;
        let temperature = self.temperature;
        let backend = Arc::clone(&self.backend);
        let input = text.to_owned();
        let voice = voice.to_owned();

        let handle: JoinHandle<()> = std::thread::spawn(move || {
            let mut sink = |chunk: Vec<f32>| -> Result<()> {
                if cancel.load(Ordering::Relaxed) {
                    return Err(anyhow!("cancelled"));
                }
                // Le canal peut être fermé si le lecteur a lâché le stream :
                // ce n'est pas une erreur du moteur, la génération s'interrompt.
                tx.send(chunk).map_err(|_| anyhow!("reader gone"))
            };
            if let Err(err) = backend.generate(&input, &voice, options, temperature, &mut sink) {
                tracing::error!("[voice] Pocket Rust generation stopped: {err}");
            }
        });

        Ok(Stream {
            rx,
            stop,
            handle: Some(handle),
        })
    }
}

/// Contexte de stream : lit les chunks produits par le thread de génération.
pub struct Stream {
    rx: Receiver<Vec<f32>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Stream {
    /// Renvoie le prochain chunk, en bloquant jusqu'à ce qu'il soit disponible.
    ///
    /// `None` signale la fin du stream (ou une annulation). Après [`Stream::stop`],
    /// la lecture est terminée : plus aucun chunk n'est rendu, même si le
    /// producteur en avait mis en attente.
    // Le nom `next` est celui d'un iterateur, mais le stream n'en est pas un :
    // il rend des chunks audio, pas des elements de collection.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Vec<f32>> {
        if self.stop.load(Ordering::Relaxed) {
            return None;
        }
        loop {
            match self.rx.recv_timeout(Duration::from_millis(100)) {
                Ok(chunk) => {
                    // Le stream peut avoir été arrêté entre-temps : le chunk
                    // restant est jeté, pas rendu.
                    if self.stop.load(Ordering::Relaxed) {
                        return None;
                    }
                    return Some(chunk);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if self.stop.load(Ordering::Relaxed) {
                        return None;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return None,
            }
        }
    }

    /// Interrompt la génération et attend la fin du thread.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Vérifie qu'un pack est complet avant de construire un moteur.
///
/// Un pack incomplet doit être refusé ici : l'appelant voit alors un moteur
/// nul et peut recharger un autre pack, au lieu de découvrir le problème à la
/// première synthèse.
fn check_pack(models: &str, voices: &str, tokenizer: &str) -> Result<()> {
    let models_path = Path::new(models);
    if !models_path.is_dir() {
        return Err(anyhow!("pack models directory missing: {models}"));
    }
    let voices_path = Path::new(voices);
    if !voices_path.is_dir() {
        return Err(anyhow!("pack voices directory missing: {voices}"));
    }
    let tokenizer_path = Path::new(tokenizer);
    if !tokenizer_path.is_file() {
        return Err(anyhow!("pack tokenizer missing: {tokenizer}"));
    }
    Ok(())
}

/// Constructeur de canal exposé pour les tests.
#[cfg(test)]
fn channel() -> (std::sync::mpsc::SyncSender<Vec<f32>>, Receiver<Vec<f32>>) {
    sync_channel(8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_options_reject_impossible_values() {
        let mut engine = Engine::with_backend(
            Arc::new(crate::backend::Stub::new("m", "v", "t", "f32", 1, 2)),
            0.5,
        );
        assert!(engine.set_text_options(-2, false, true, 50).is_err());
        assert!(engine.set_text_options(8, false, true, -1).is_err());
        assert!(engine.set_text_options(8, false, true, 50).is_ok());
    }

    #[test]
    fn text_options_freeze_after_the_first_stream() {
        let mut engine = Engine::with_backend(
            Arc::new(crate::backend::Stub::new("m", "v", "t", "f32", 1, 2)),
            0.5,
        );
        engine.set_text_options(8, false, true, 50).expect("set");
        assert!(engine.start("bonjour", "voice.wav").is_ok());
        assert!(engine.set_text_options(-1, true, false, 0).is_err());
    }

    #[test]
    fn a_pack_without_its_files_is_refused() {
        let err = match Engine::load(
            "nowhere",
            "nowhere/voices",
            "nowhere/tokenizer.json",
            "f32",
            0.5,
            1,
            2,
        ) {
            Ok(_) => panic!("un pack absent doit être refusé"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("models directory missing"),
            "{err}"
        );
    }

    #[test]
    fn a_bounded_channel_survives_a_slow_reader() {
        let (tx, rx) = channel();
        for i in 0..4 {
            tx.send(vec![i as f32; CHUNK]).expect("send");
        }
        drop(tx);
        let mut total = 0;
        while let Ok(chunk) = rx.recv() {
            total += chunk.len();
        }
        assert_eq!(total, 4 * CHUNK);
    }
}
