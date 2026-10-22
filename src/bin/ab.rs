//! Comparatif A/B : le nombre de frames rendues après l'EOS.
//!
//! Le moteur `ptts` rend 1 ou 3 frames après l'EOS, selon la longueur de la
//! phrase. Le français 24 couches de Kyutai, dans sa version 3.1.0, en
//! demandait 8 ; le pack FR d'Unifia applique encore 8 aujourd'hui, via
//! `text_options_for("fr")`.
//!
//! Ce programme rend la même phrase, avec la même graine, pour plusieurs
//! nombres de frames (1, 3, 8 par défaut), et écrit un WAV par rendu, avec
//! ses métriques. Les rendus sont ensuite comparés à l'oreille et transcrits.
//!
//! # Usage
//!
//! ```text
//! cargo run --release --bin ab -- \
//!   --pack D:/App/unifia/models/fr \
//!   --voice estelle \
//!   --frames 1,3,8 \
//!   --out D:/App/unifia/models/fr/ab
//! ```

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{bail, Context, Result};

use pocket_tts_mobile::engine::Engine;

/// Les phrases du jeu de test français, si aucune n'est fournie en ligne de
/// commande.
const DEFAULT_TEXTS: [&str; 3] = [
    "Bonjour le monde.",
    "Je vais bien, merci beaucoup.",
    "La dictée vocale est maintenant disponible en français sur ce téléphone, et elle respecte les phrases longues sans les couper.",
];

struct Args {
    pack: PathBuf,
    voice: String,
    out: PathBuf,
    texts: Vec<String>,
    frames: Vec<i32>,
    temperature: f32,
    seed: u64,
}

fn parse_args() -> Result<Args> {
    let mut pack = PathBuf::from("../models/fr");
    let mut voice = "estelle".to_string();
    let mut out = PathBuf::from("ab");
    let mut texts: Vec<String> = Vec::new();
    let mut frames: Vec<i32> = vec![1, 3, 8];
    let mut temperature = 0.5f32;
    let mut seed = 4242424242424242u64;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value =
            || -> Result<String> { args.next().with_context(|| format!("{arg} needs a value")) };
        match arg.as_str() {
            "--pack" => pack = PathBuf::from(value()?),
            "--voice" => voice = value()?,
            "--out" => out = PathBuf::from(value()?),
            "--text" => texts.push(value()?),
            "--frames" => {
                frames = value()?
                    .split(',')
                    .map(|part| part.trim().parse::<i32>())
                    .collect::<std::result::Result<Vec<i32>, _>>()
                    .context("frames must be a comma separated list of numbers")?;
            }
            "--temperature" => temperature = value()?.parse().context("bad temperature")?,
            "--seed" => seed = value()?.parse().context("bad seed")?,
            other => bail!("unknown argument {other}"),
        }
    }
    if texts.is_empty() {
        texts = DEFAULT_TEXTS.iter().map(|t| t.to_string()).collect();
    }
    Ok(Args {
        pack,
        voice,
        out,
        texts,
        frames,
        temperature,
        seed,
    })
}

fn main() -> Result<()> {
    let args = parse_args()?;
    std::fs::create_dir_all(&args.out).ok();
    std::env::set_var("POCKET_TTS_SEED", args.seed.to_string());

    let mut engine = Engine::load(
        &args.pack.to_string_lossy(),
        &args.pack.join("embeddings").to_string_lossy(),
        &args.pack.join("tokenizer.json").to_string_lossy(),
        "auto",
        args.temperature,
        1,
        2,
    )?;
    engine.warmup().context("the warmup failed")?;

    let mut index = Vec::new();
    for text in &args.texts {
        for frames in &args.frames {
            // Use the model's default text options (from the config), which are correct for French.
            // The frames_after_eos is passed separately to the generation function.
            // No need to call set_text_options - the model uses its default text options from the config.
            let start = Instant::now();
            let mut first = None;
            let mut total = 0usize;
            let mut pcm = Vec::new();
            {
                let mut stream = engine
                    .start(text, &args.voice)
                    .context("the generation did not start")?;
                while let Some(chunk) = stream.next() {
                    if first.is_none() {
                        first = Some(start.elapsed().as_secs_f64() * 1000.0);
                    }
                    total += chunk.len();
                    pcm.extend_from_slice(&chunk);
                }
            }
            let elapsed = start.elapsed().as_secs_f64();
            let duration = total as f64 / 24_000.0;
            let rtf = if duration > 0.0 {
                elapsed / duration
            } else {
                0.0
            };
            let ttfa = first.unwrap_or(0.0);
            let rms = if pcm.is_empty() {
                0.0
            } else {
                (pcm.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / pcm.len() as f64)
                    .sqrt()
            };
            println!(
                "frames={frames} ttfa={ttfa:.0}ms rtf={rtf:.3} duration={duration:.2}s rms={rms:.5} | {text}"
            );
            let name = format!("f{frames}-{}.wav", rows_slug(text));
            let path = args.out.join(&name);
            ptts::wav::write_wav_file(&path, &pcm, 24_000)
                .with_context(|| format!("cannot write {}", path.display()))?;
            index.push(format!(
                "{name}\tframes={frames}\tttfa={ttfa:.0}\trtf={rtf:.3}\tduration={duration:.2}\trms={rms:.5}\t{text}"
            ));
        }
    }
    std::fs::write(args.out.join("index.tsv"), index.join("\n")).ok();
    println!("wrote {} renders in {}", index.len(), args.out.display());
    Ok(())
}

fn rows_slug(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    for c in text.chars().take(24) {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if c.is_whitespace() {
            slug.push('_');
        }
    }
    if slug.is_empty() {
        slug.push_str("text");
    }
    slug
}
