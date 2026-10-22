//! Comparatif A/B : chemin stock contre chemin bas niveau.
//!
//! # Ce que ce programme teste
//!
//! Le moteur `ptts` calcule en interne le nombre de frames rendues apres l'EOS
//! (3 pour un texte court, 1 sinon). Le francais 24 couches de Kyutai, lui, a
//! ete exporte avec `model_recommended_frames_after_eos: 8` (version 3.1.0),
//! et le pack FR d'Unifia rend 8 frames. C'est la cause du defaut "mot final
//! avale" : sur 18 rendus int8 transcrits, la phrase d'ouverture etait perdue
//! dans 6 cas sur 6 avec 3 frames, et conservee 6 fois sur 6 avec 8 ; WER moyen
//! 0.101 contre 0.048.
//!
//! Ce programme rend la meme phrase par les deux chemins, avec la meme graine :
//!
//! - **stock** : `Synth::stream_with`, le chemin de haut niveau. Les frames
//!   sont calculees a l'interieur, sans possibilite de les changer.
//! - **bas niveau** : `Session::stream_tokens`, qui prend les tokens et le
//!   nombre de frames. Le texte est prepare par le front-end Kyutai, puis
//!   tokenise, puis synthetise avec le nombre de frames demande.
//!
//! Les deux rendus sont ecrits en WAV, avec leurs metriques, pour etre
//! compares a l'oreille et transcrits ensuite.
//!
//! # Usage
//!
//! ```text
//! cargo run --release --bin ab -- \
//!   --pack D:/App/unifia/models/fr \
//!   --text "Bonjour le monde." \
//!   --frames 1,3,8 \
//!   --voice estelle \
//!   --out D:/App/unifia/models/fr/ab
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use ptts::checkpoint::{Checkpoint, ResolveOptions};
use ptts::flow_lm::NormalRng;
use ptts::preprocess::{Lang, Normalize};
use ptts::synth::{DeviceKind, Quant, SpeechOptions, Synth};

/// Textes du jeu de test francais, si aucun n'est fourni en ligne de commande.
const DEFAULT_TEXTS: [&str; 3] = [
    "Bonjour le monde.",
    "Je vais bien, merci beaucoup.",
    "La dictée vocale est maintenant disponible en français sur ce téléphone, et elle respecte les phrases longues sans les couper.",
];

/// Options de la ligne de commande, sans dependance externe.
struct Args {
    pack: PathBuf,
    texts: Vec<String>,
    frames: Vec<usize>,
    voice: Option<String>,
    temperature: f32,
    seed: u64,
    out: PathBuf,
    lang: Lang,
}

fn parse_args() -> Result<Args> {
    let mut pack = PathBuf::from("../models/fr");
    let mut texts: Vec<String> = Vec::new();
    let mut frames: Vec<usize> = vec![1, 3, 8];
    let mut voice = None;
    let mut temperature = 0.5f32;
    let mut seed = 4242424242424242u64;
    let mut out = PathBuf::from("ab");
    let mut lang = Lang::Fr;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || -> Result<String> {
            args.next().with_context(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--pack" => pack = PathBuf::from(value()?),
            "--text" => texts.push(value()?),
            "--frames" => {
                frames = value()?
                    .split(',')
                    .map(|part| part.trim().parse::<usize>())
                    .collect::<std::result::Result<Vec<usize>, _>>()
                    .context("frames must be a comma separated list of numbers")?;
            }
            "--voice" => voice = Some(value()?),
            "--temperature" => temperature = value()?.parse().context("bad temperature")?,
            "--seed" => seed = value()?.parse().context("bad seed")?,
            "--out" => out = PathBuf::from(value()?),
            "--lang" => {
                lang = match value()?.as_str() {
                    "fr" => Lang::Fr,
                    "en" => Lang::En,
                    other => bail!("unsupported language {other}"),
                }
            }
            other => bail!("unknown argument {other}"),
        }
    }
    if texts.is_empty() {
        texts = DEFAULT_TEXTS.iter().map(|t| t.to_string()).collect();
    }
    Ok(Args {
        pack,
        texts,
        frames,
        voice,
        temperature,
        seed,
        out,
        lang,
    })
}

fn main() -> Result<()> {
    let args = parse_args()?;
    std::fs::create_dir_all(&args.out).ok();

    // 1. Le checkpoint.
    let checkpoint = Checkpoint::locate(
        args.pack.to_str().context("pack path is not UTF-8")?,
        ResolveOptions {
            quant: Quant::F32,
            weights: None,
        },
    )
    .with_context(|| format!("cannot locate the checkpoint in {}", args.pack.display()))?;
    println!(
        "checkpoint: weights={} tokenizer={:?} voices={:?}",
        checkpoint.weights.display(),
        checkpoint.tokenizer,
        checkpoint.voices.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );

    // 2. Le moteur.
    let load_start = std::time::Instant::now();
    let synth: Synth = checkpoint
        .builder(Normalize::for_lang(args.lang))
        .device(DeviceKind::Cpu)
        .quant(Quant::F32)
        .temperature(args.temperature)
        .seed(args.seed)
        .build()
        .context("cannot build the synth")?;
    println!(
        "loaded in {:.0} ms: sample_rate={} voices={:?}",
        load_start.elapsed().as_secs_f64() * 1e3,
        synth.sample_rate(),
        synth.voices()
    );

    // 3. La voix demandee, si elle existe dans le pack.
    let voice = match &args.voice {
        Some(want) => {
            let known = synth.voices();
            if !known.iter().any(|name| name == want) {
                bail!("voice {want} is not in the pack, which has {known:?}");
            }
            Some(want.clone())
        }
        None => None,
    };

    let mut report = Vec::new();
    for text in &args.texts {
        // --- chemin stock : les frames sont calculees a l'interieur.
        let mut opts = SpeechOptions::default()
            .temperature(args.temperature)
            .seed(args.seed);
        if let Some(name) = &voice {
            opts = opts.voice(name.clone());
        }
        let stock = render_stock(&synth, text, &opts)?;
        println!(
            "stock | {} | {:.2}s | rms={:.5} peak={:.4}",
            text,
            stock.duration_s,
            stock.rms,
            stock.peak
        );
        report.push(Report {
            text: text.clone(),
            path: "stock".to_owned(),
            frames: None,
            ..stock
        });

        // --- chemin bas niveau : le nombre de frames est choisi ici.
        for frames in &args.frames {
            let low = render_low_level(&synth, text, &voice, *frames, args.temperature, args.seed)?;
            println!(
                "frames={frames} | {} | {:.2}s | rms={:.5} peak={:.4}",
                text, low.duration_s, low.rms, low.peak
            );
            report.push(Report {
                text: text.clone(),
                path: format!("frames-{frames}"),
                frames: Some(*frames),
                ..low
            });
        }
    }

    // 4. Les WAV, pour l'ecoute et la transcription.
    let mut index = Vec::new();
    for (i, item) in report.iter().enumerate() {
        let name = format!("{i:02}-{}.wav", item.path);
        let path = args.out.join(&name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        ptts::wav::write_wav_file(&path, &item.pcm, 24_000)
            .with_context(|| format!("cannot write {}", path.display()))?;
        index.push(format!(
            "{}\t{}\tframes={:?}\tduration={:.2}\trms={:.5}\tpeak={:.4}",
            name, item.text, item.frames, item.duration_s, item.rms, item.peak
        ));
    }
    let index_path = args.out.join("index.tsv");
    std::fs::write(&index_path, index.join("\n")).ok();
    println!("wrote {} renders in {}", report.len(), args.out.display());
    Ok(())
}

/// Metriques d'un rendu.
struct Report {
    text: String,
    path: String,
    frames: Option<usize>,
    pcm: Vec<f32>,
    duration_s: f64,
    rms: f64,
    peak: f32,
}

/// Rend par le chemin de haut niveau, celui du FFI stock.
fn render_stock(synth: &Synth, text: &str, opts: &SpeechOptions) -> Result<Report> {
    let start = std::time::Instant::now();
    let mut pcm = Vec::new();
    let stream = synth
        .stream_with(text, opts)
        .context("cannot start the stock generation")?;
    for chunk in stream {
        let chunk = chunk.context("stock generation failed")?;
        pcm.extend_from_slice(&chunk);
    }
    let elapsed = start.elapsed().as_secs_f64();
    Ok(metrics(text, "stock", None, pcm, elapsed))
}

/// Rend par le chemin de bas niveau, qui accepte le nombre de frames.
fn render_low_level(
    synth: &Synth,
    text: &str,
    voice: &Option<String>,
    frames: usize,
    temperature: f32,
    seed: u64,
) -> Result<Report> {
    // La session est primée avec la voix : c'est elle qui porte le conditionnement.
    let mut opts = SpeechOptions::default().temperature(temperature).seed(seed);
    if let Some(name) = voice {
        opts = opts.voice(name.clone());
    }
    let session = synth
        .session_default(&opts)
        .context("cannot prime a session")?;

    // Le texte est prepare par le front-end Kyutai, pas par celui du moteur.
    let options = pocket_tts_mobile::kyutai::text::TextOptions {
        pad_with_spaces_for_short_inputs: false,
        remove_semicolons: true,
        append_terminal_punctuation: true,
        capitalize_first_letter: true,
        replace_characters: Default::default(),
    };
    let (prepared, _guess) = pocket_tts_mobile::kyutai::text::prepare_text_prompt(text, &options)
        .context("cannot prepare the text")?;

    let tokens = session.tokenize(&prepared).context("cannot tokenize")?;
    let rng = Box::new(NormalRng::new(temperature, seed).context("cannot build the noise")?);
    let start = std::time::Instant::now();
    let mut pcm = Vec::new();
    let stream = session
        .stream_tokens(tokens, frames, rng)
        .context("cannot start the low level generation")?;
    for chunk in stream {
        let chunk = chunk.context("low level generation failed")?;
        pcm.extend_from_slice(&chunk);
    }
    let elapsed = start.elapsed().as_secs_f64();
    Ok(metrics(text, "low", Some(frames), pcm, elapsed))
}

/// Calcule les metriques d'un rendu.
fn metrics(text: &str, path: &str, frames: Option<usize>, pcm: Vec<f32>, elapsed: f64) -> Report {
    let duration_s = if pcm.is_empty() {
        0.0
    } else {
        pcm.len() as f64 / 24_000.0
    };
    let rms = if pcm.is_empty() {
        0.0
    } else {
        (pcm.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / pcm.len() as f64).sqrt()
    };
    let peak = pcm.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let _ = elapsed;
    Report {
        text: text.to_owned(),
        path: path.to_owned(),
        frames,
        pcm,
        duration_s,
        rms,
        peak,
    }
}
