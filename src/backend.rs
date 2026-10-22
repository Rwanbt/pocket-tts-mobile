//! Backends de synthèse — Pocket TTS Mobile.
//!
//! # Deux backends
//!
//! - [`Kyutai`] : le runtime officiel de Pocket TTS, vendorisé dans
//!   [`crate::kyutai`]. C'est celui qui compte : il sait lire les voix que
//!   Kyutai publie, que `ptts::synth` rejette silencieusement.
//! - [`Stub`] : un bouchon qui refuse de synthétiser. Il permet d'exercer et
//!   de tester l'ABI sans les poids, et sert de témoin dans les tests.
//!
//! # Pourquoi pas `ptts::synth`
//!
//! `ptts::synth::Synth` est l'API de haut niveau du moteur. Elle.refait
//! l'orchestration de Kyutai autrement, et en pire : les voix publiées par
//! Kyutai sont des états de cache KV, pas des embeddings, et elle les rejette
//! dans un `tracing::warn!` avalé. Le modèle se charge, la liste des voix est
//! vide, et la génération se fait sans condition vocale.
//!
//! Le runtime de Kyutai, lui, lit ces états : c'est `weights.rs` qui sait
//! les importer. Le moteur, en dessous, reste le même
//! (`ptts::tts_model::TTSModel`), donc il n'y a pas de duplication.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use xn::quantized::Q80F32;
use xn::{CpuDevice, Unquantized};

use crate::engine::TextOptions;
use crate::kyutai::config::Config;
use crate::kyutai::model::{CpuBackend, LoadOptions, TtsModel, VoiceState};

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
        sink: &mut (dyn FnMut(Vec<f32>) -> Result<()> + Send),
    ) -> Result<()>;
}

// ============================================================================
// Le runtime Kyutai
// ============================================================================

/// Le moteur Kyutai, pour une précision de poids donnée.
///
/// Le type de précision est un paramètre générique dans le runtime d'origine ;
/// il est effacé derrière [`Runner`] pour que l'appelant puisse choisir au
/// démarrage, comme le fait l'argument `precision` de `ptt_create`.
pub struct Runtime<Q: CpuBackend> {
    /// Le modele et les etats de voix, sous un seul verrou.
    ///
    /// Un seul verrou, plutot qu'un par structure : la generation utilise les
    /// deux en meme temps, et deux verrous distincts obligeraient a fixer
    /// l'ordre d'acquisition, donc a le maintenir.
    inner: Mutex<Inner<Q>>,
    /// Le dossier des voix du pack.
    voices_dir: PathBuf,
}

/// Le modele et ses etats de voix.
struct Inner<Q: CpuBackend> {
    model: TtsModel<Q>,
    voices: HashMap<String, VoiceState<Q>>,
}

/// Le moteur Kyutai, sans le type de précision.
pub struct Kyutai {
    inner: Box<dyn Runner>,
}

/// Ce que le moteur doit savoir faire, indépendamment de la précision.
pub trait Runner: Send + Sync {
    /// La fréquence d'échantillonnage du codec.
    fn sample_rate(&self) -> usize;

    /// Prépare le moteur (charge les poids).
    fn warmup(&self) -> Result<()>;

    /// Synthétise et pousse les chunks dans `sink`.
    ///
    /// La temperature n'est pas un parametre d'appel : elle est fixee au
    /// chargement du modele, comme dans le runtime d'origine.
    fn generate(
        &self,
        voice: &str,
        text: &str,
        options: TextOptions,
        _temperature: f32,
        stop: &AtomicBool,
        sink: &mut (dyn FnMut(&[f32]) -> Result<()> + Send),
    ) -> Result<()>;
}

impl Kyutai {
    /// Charge un pack et prépare le moteur.
    ///
    /// `config` est la config Kyutai du modele (YAML), qui porte les options
    /// du front-end texte : le `config.json` de `ptts` ne les contient pas, et
    /// le runtime en a besoin.
    ///
    /// `precision` n'a pas besoin d'etre passe : il se deduit du fichier de
    /// poids, le runtime d'origine ne sachant pas melanger les deux dans un
    /// meme processus.
    ///
    /// Sept arguments sont necessaires : le moteur a besoin de chacun d'eux,
    /// et les regrouper dans une structure ne ferait que deplacer la
    /// complexite sans la reduire.
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        config: Config,
        origin: crate::kyutai::config::Origin,
        models_dir: &Path,
        voices_dir: &Path,
        weights: &Path,
        tokenizer: &Path,
        temperature: f32,
        eos_threshold: Option<f32>,
    ) -> Result<Self> {
        let options = LoadOptions {
            temperature: Some(temperature),
            sampler_decode_steps: 1,
            noise_clamp: None,
            eos_threshold,
        };
        // Les poids et le tokenizer viennent du pack, pas du depot Hugging
        // Face : la config YAML porte des URLs `hf://`, qui doivent etre
        // remplacees par des chemins locaux avant le chargement.
        let mut config = config;
        config.weights_path = Some(weights.to_string_lossy().to_string());
        config.weights_path_without_voice_cloning = None;
        config.flow_lm.lookup_table.tokenizer_path = tokenizer.to_string_lossy().to_string();
        let _ = models_dir;

        let inner: Box<dyn Runner> = match precision_of(weights) {
            Precision::F32 => Box::new(Runtime::<Unquantized<f32, CpuDevice>>::load(
                config,
                origin,
                options,
                voices_dir.to_path_buf(),
            )?),
            Precision::Q8 => Box::new(Runtime::<Q80F32>::load(
                config,
                origin,
                options,
                voices_dir.to_path_buf(),
            )?),
        };
        Ok(Self { inner })
    }
}

/// La precision des poids d'un pack.
enum Precision {
    F32,
    Q8,
}

/// Deduit la precision du pack de l'extension du fichier de poids.
fn precision_of(weights: &Path) -> Precision {
    match weights.extension().and_then(|ext| ext.to_str()) {
        Some("gguf") => Precision::Q8,
        _ => Precision::F32,
    }
}

impl<Q: CpuBackend + Send + Sync> Runtime<Q> {
    /// Charge le modele et prepare le dossier des voix.
    fn load(
        config: Config,
        origin: crate::kyutai::config::Origin,
        options: LoadOptions,
        voices_dir: PathBuf,
    ) -> Result<Self> {
        let model = TtsModel::load(config, origin, &options)
            .context("cannot load the Pocket model from the pack")?;
        Ok(Self {
            inner: Mutex::new(Inner {
                model,
                voices: HashMap::new(),
            }),
            voices_dir,
        })
    }
}

impl<Q: CpuBackend + Send + Sync> Runner for Runtime<Q> {
    fn sample_rate(&self) -> usize {
        self.inner
            .lock()
            .map(|inner| inner.model.sample_rate())
            .unwrap_or(24_000)
    }

    fn warmup(&self) -> Result<()> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("model poisoned"))?;
        let _ = inner.model.sample_rate();
        Ok(())
    }

    fn generate(
        &self,
        voice: &str,
        text: &str,
        options: TextOptions,
        _temperature: f32,
        stop: &AtomicBool,
        sink: &mut (dyn FnMut(&[f32]) -> Result<()> + Send),
    ) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("model poisoned"))?;
        // Le runtime utilise les options de texte de sa config ; celles de
        // l'appelant doivent les remplacer, sans quoi le francais perdrait le
        // traitement des points-virgules.
        let text_options =
            crate::eos::effective_text_options(inner.model.text_options(), &options);
        inner.model.set_text_options(text_options);
        // La voix est chargee au premier usage, puis conservee : la relire a
        // chaque enonce coûterait 28 Mo de lecture par phrase.
        if !inner.voices.contains_key(voice) {
            let path = self.voices_dir.join(voice);
            if !path.is_file() {
                bail!(
                    "voice {voice:?} is not in the pack, which has {:?}",
                    crate::voice::list(&self.voices_dir)
                );
            }
            let state = inner
                .model
                .get_state_for_audio_prompt(&path.to_string_lossy(), false)
                .with_context(|| format!("cannot load the voice {voice:?}"))?;
            inner.voices.insert(voice.to_owned(), state);
        }
        let state = inner
            .voices
            .get(voice)
            .expect("the voice was just inserted");
        // Le nombre de frames apres l'EOS : la valeur de l'appelant si elle
        // existe, sinon celle que la config recommande, sinon l'estimation du
        // front-end. C'est la regle de `crate::eos`.
        let frames = if options.eos_extra_frames >= 0 {
            Some(options.eos_extra_frames as usize)
        } else {
            None
        };
        inner
            .model
            .generate_audio_stream(
                state,
                text,
                options.max_chunk_tokens.max(1) as usize,
                frames,
                stop,
                sink,
            )
            .context("the Pocket generation failed")
    }
}

impl Backend for Kyutai {
    fn warmup(&self) -> Result<()> {
        self.inner.warmup()
    }

    fn generate(
        &self,
        text: &str,
        voice: &str,
        options: TextOptions,
        temperature: f32,
        sink: &mut (dyn FnMut(Vec<f32>) -> Result<()> + Send),
    ) -> Result<()> {
        let stop = AtomicBool::new(false);
        let mut forward = |pcm: &[f32]| -> Result<()> { sink(pcm.to_vec()) };
        self.inner
            .generate(voice, text, options, temperature, &stop, &mut forward)
    }
}

// ============================================================================
// Le bouchon
// ============================================================================

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
        _sink: &mut (dyn FnMut(Vec<f32>) -> Result<()> + Send),
    ) -> Result<()> {
        bail!("{NOT_IMPLEMENTED} (text={text:?}, voice={voice:?})")
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

    #[test]
    fn the_precision_follows_the_weights_file() {
        assert!(matches!(
            precision_of(&PathBuf::from("model.safetensors")),
            Precision::F32
        ));
        assert!(matches!(
            precision_of(&PathBuf::from("model.q8.gguf")),
            Precision::Q8
        ));
    }
}
