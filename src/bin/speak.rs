//! Genere un WAV francais avec le moteur Rust `ptts` depuis un pack modele local.
//!
//! C'est le spike Phase 1 : le meme chemin de code que la cdylib Android
//! (`ptts::checkpoint::Checkpoint` -> `ptts::synth::SynthBuilder` -> `ptts::synth::Synth`),
//! mais sur PC, avec un WAV en sortie.
//!
//! Usage :
//!   cargo run --release --bin speak -- \
//!     --pack D:/App/unifia/models/fr --text "Bonjour le monde." --out out.wav
//!
//! Options :
//!   --voice <wav>   fichier audio (~10 s) pour cloner une voix ; sans lui, la
//!                   generation est sans condition vocale (voix apprise du modele).
//!   --no-normalize  ne pas appliquer la normalisation texte de ptts.

use anyhow::{bail, Context, Result};
use ptts::checkpoint::{Checkpoint, ResolveOptions};
use ptts::preprocess::{Lang, Normalize};
use ptts::synth::{DeviceKind, Quant, SpeechOptions, Synth};
use std::path::PathBuf;
use std::time::Instant;

fn main() -> Result<()> {
    let mut pack = PathBuf::from("../models/fr");
    let mut text = "Bonjour le monde.".to_string();
    let mut out = PathBuf::from("out.wav");
    let mut voice: Option<PathBuf> = None;
    let mut lang: Option<String> = None;
    let mut temperature = 0.3f32;
    let mut seed: u64 = 4242424242424242;
    let mut quant = Quant::F32;
    let mut register_voices = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value =
            || -> Result<String> { args.next().with_context(|| format!("{arg} needs a value")) };
        match arg.as_str() {
            "--pack" => pack = PathBuf::from(value()?),
            "--text" => text = value()?,
            "--out" => out = PathBuf::from(value()?),
            "--voice" => voice = Some(PathBuf::from(value()?)),
            "--lang" => lang = Some(value()?),
            "--temperature" => temperature = value()?.parse().context("bad --temperature")?,
            "--seed" => seed = value()?.parse().context("bad --seed")?,
            "--register-voices" => register_voices = true,
            "--quant" => {
                quant = match value()?.as_str() {
                    "f32" => Quant::F32,
                    "q8" | "q8_0" => Quant::Q80,
                    other => bail!("unsupported --quant {other:?}"),
                }
            }
            other => bail!("unknown argument {other:?}"),
        }
    }

    let lang: Lang = match lang.as_deref() {
        Some("fr") | None => Lang::Fr,
        Some("en") => Lang::En,
        Some("none") => bail!("--lang none is not supported by this spike"),
        Some(other) => bail!("unsupported language {other:?}"),
    };

    // 1. Resoudre le pack : config.json + poids + tokenizer.
    let checkpoint = Checkpoint::locate(
        pack.to_str().context("the pack path is not valid UTF-8")?,
        ResolveOptions {
            quant,
            weights: None,
        },
    )
    .with_context(|| format!("cannot locate the checkpoint in {}", pack.display()))?;
    println!(
        "checkpoint: weights={} quant={} tokenizer={:?} voices={:?}",
        checkpoint.weights.display(),
        checkpoint.quant.as_str(),
        checkpoint.tokenizer,
        checkpoint.voices.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );

    // 2. Construire le moteur.
    let load_start = Instant::now();
    let builder = checkpoint
        .builder(Normalize::for_lang(lang))
        .device(DeviceKind::Cpu)
        .quant(quant)
        .temperature(temperature)
        .seed(seed);
    let mut synth: Synth = builder.build().context("cannot build the synth")?;
    if register_voices {
        // Comme `ptts-ffi` : les voix du pack (dossier `voices/` ou `embeddings/`)
        // sont enregistrees apres le chargement. `Checkpoint::register_voices`
        // avale l'echec dans un `tracing::warn!` ; on appelle donc
        // `add_voice_file` directement pour voir l'erreur.
        for (name, path) in &checkpoint.voices {
            match synth.add_voice_file(name, path) {
                Ok(()) => println!("voice {name:?} registered"),
                Err(e) => println!("voice {name:?} REJECTED: {e}"),
            }
        }
    }
    let load_ms = load_start.elapsed().as_secs_f64() * 1e3;
    println!(
        "loaded in {load_ms:.0} ms: device={} sample_rate={} voices={:?} languages={:?}",
        synth.device_name(),
        synth.sample_rate(),
        synth.voices(),
        synth.languages()
    );

    // 3. Voix : clonage depuis un audio, ou generation sans condition vocale.
    let mut opts = SpeechOptions::default().temperature(temperature).seed(seed);
    if let Some(path) = &voice {
        let pcm = ptts::audio::load_mono_at(path, synth.voice_prompt_sample_rate())
            .with_context(|| format!("cannot read the voice prompt {}", path.display()))?;
        println!(
            "voice prompt: {} samples at {} Hz",
            pcm.len(),
            synth.voice_prompt_sample_rate()
        );
        synth
            .add_voice_from_pcm("spike", &pcm)
            .context("cannot register the voice")?;
        opts = opts.voice("spike");
    }

    // 4. Generer.
    let gen_start = Instant::now();
    let stream = synth
        .stream_with(&text, &opts)
        .context("cannot start the generation")?;
    let sample_rate = stream.sample_rate();
    let mut pcm: Vec<f32> = Vec::new();
    let mut ttfa_ms: Option<f64> = None;
    for chunk in stream {
        let chunk = chunk.context("generation failed")?;
        if chunk.is_empty() {
            continue;
        }
        ttfa_ms.get_or_insert_with(|| gen_start.elapsed().as_secs_f64() * 1e3);
        pcm.extend_from_slice(&chunk);
    }
    let gen_ms = gen_start.elapsed().as_secs_f64() * 1e3;
    let duration_s = pcm.len() as f64 / sample_rate as f64;
    let rtf = if duration_s > 0.0 {
        gen_ms / 1000.0 / duration_s
    } else {
        f64::NAN
    };

    println!(
        "generated {:.2}s of audio in {:.0} ms (RTF={rtf:.3}, TTFA={:.0} ms), {} samples at {sample_rate} Hz",
        duration_s,
        gen_ms,
        ttfa_ms.unwrap_or(0.0),
        pcm.len()
    );
    if pcm.is_empty() {
        bail!("the generation produced no audio");
    }
    let rms =
        (pcm.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / pcm.len() as f64).sqrt();
    let peak = pcm.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    println!("audio: RMS={rms:.5} peak={peak:.4}");

    ptts::wav::write_wav_file(&out, &pcm, sample_rate)
        .with_context(|| format!("cannot write {}", out.display()))?;
    println!("wrote {}", out.display());
    Ok(())
}
