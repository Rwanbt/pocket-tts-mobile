//! Banc de mesure du runtime Pocket TTS.
//!
//! Rend un jeu de phrases avec le moteur de production (le `Backend` de
//! l'ABI `ptt_*`), et rapporte les mesures qui decides du choix du pack :
//!
//! - **temps de chargement** : `ptt_create` + `ptt_warmup` ;
//! - **TTFA** : temps jusqu'au premier chunk audio ;
//! - **RTF** : temps de generation / duree de l'audio ; en dessous de 1, la
//!   generation est plus rapide que le temps reel ;
//! - **RSS** : memoire residente maximale, lue dans `/proc/self/status`.
//!
//! # Usage
//!
//! ```text
//! cargo run --release --bin bench -- \
//!   --pack D:/App/unifia/models/fr \
//!   --voice estelle \
//!   --out D:/App/unifia/models/fr/bench
//! ```
//!
//! Les phrases viennent de `tests/fr_benchmark.json` quand il est present.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail};

use pocket_tts_mobile::engine::Engine;

/// Les phrases du jeu de test, si le fichier n'est pas disponible.
const FALLBACK: [&str; 4] = [
    "Bonjour le monde.",
    "Je vais bien, merci beaucoup.",
    "La dictée vocale est maintenant disponible en français sur ce téléphone, et elle respecte les phrases longues sans les couper.",
    "Le montant total est de 123 euros et 45 centimes, pour la commande numéro 7890.",
];

/// Les options de la ligne de commande, sans dependance externe.
struct Args {
    pack: PathBuf,
    voice: String,
    out: PathBuf,
    frames: i32,
    temperature: f32,
    seed: u64,
}

fn parse_args() -> Result<Args> {
    let mut pack = PathBuf::from("../models/fr");
    let mut voice = "estelle".to_string();
    let mut out = PathBuf::from("bench");
    let mut frames = -1;
    let mut temperature = 0.5f32;
    let mut seed = 4242424242424242u64;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || -> Result<String> {
            args.next().with_context(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--pack" => pack = PathBuf::from(value()?),
            "--voice" => voice = value()?,
            "--out" => out = PathBuf::from(value()?),
            "--frames" => frames = value()?.parse().context("bad frames")?,
            "--temperature" => temperature = value()?.parse().context("bad temperature")?,
            "--seed" => seed = value()?.parse().context("bad seed")?,
            other => bail!("unknown argument {other}"),
        }
    }
    Ok(Args {
        pack,
        voice,
        out,
        frames,
        temperature,
        seed,
    })
}

/// La memoire residente maximale, en Mo, lue dans `/proc/self/status`.
///
/// Valeur nulle quand l'information n'est pas disponible (hors Linux).
fn peak_rss_mb() -> f64 {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return 0.0;
    };
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            let kb: f64 = rest
                .trim()
                .trim_end_matches(" kB")
                .trim()
                .parse()
                .unwrap_or(0.0);
            return kb / 1024.0;
        }
    }
    0.0
}

/// Les phrases du jeu de test.
fn sentences() -> Vec<String> {
    let path = PathBuf::from("tests/fr_benchmark.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return FALLBACK.iter().map(|s| s.to_string()).collect();
    };
    // Le fichier est du JSON simple : on extrait les textes sans dependance.
    let mut out = Vec::new();
    let mut rest = raw.as_str();
    while let Some(i) = rest.find("\"text\": \"") {
        rest = &rest[i + 9..];
        if let Some(end) = rest.find('"') {
            out.push(rest[..end].to_string());
            rest = &rest[end..];
        } else {
            break;
        }
    }
    if out.is_empty() {
        return FALLBACK.iter().map(|s| s.to_string()).collect();
    }
    out
}

fn main() -> Result<()> {
    let args = parse_args()?;
    std::fs::create_dir_all(&args.out).ok();

    // La graine du bruit est fixee par le runtime (42 par defaut, comme le
    // generateur de PyTorch). Elle est imposee ici pour que deux packs
    // compares rendent le meme bruit, et que la comparaison porte sur le
    // format des poids, pas sur le hasard.
    std::env::set_var("POCKET_TTS_SEED", args.seed.to_string());

    // 1. Le chargement, mesure.
    let load_start = Instant::now();
    let mut engine = Engine::load(
        &args.pack.to_string_lossy(),
        &args.pack.join("embeddings").to_string_lossy(),
        &args.pack.join("tokenizer.json").to_string_lossy(),
        "auto",
        args.temperature,
        1,
        2,
    )?;
    engine
        .set_text_options(
            args.frames,
            false,
            true,
            50,
        )
        .context("the pack rejected the text options")?;
    engine.warmup().context("the warmup failed")?;
    let load_ms = load_start.elapsed().as_secs_f64() * 1000.0;
    println!("loaded in {load_ms:.0} ms");

    // 2. Les rendus.
    let mut rows = Vec::new();
    for text in sentences() {
        let start = Instant::now();
        let mut first = None;
        let mut total = 0usize;
        let mut pcm = Vec::new();
        {
            let mut stream = engine
                .start(&text, &args.voice)
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
        let rtf = if duration > 0.0 { elapsed / duration } else { 0.0 };
        let ttfa = first.unwrap_or(0.0);
        let rms = if pcm.is_empty() {
            0.0
        } else {
            (pcm.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / pcm.len() as f64).sqrt()
        };
        println!(
            "ttfa={ttfa:.0}ms rtf={rtf:.3} duration={duration:.2}s rms={rms:.5} | {text}"
        );
        rows.push(format!(
            "{ttfa:.0}\t{rtf:.3}\t{duration:.2}\t{rms:.5}\t{text}"
        ));
        // Le WAV, pour l'ecoute et la transcription.
        let name = format!("{}.wav", rows.len());
        let path = args.out.join(&name);
        ptts::wav::write_wav_file(&path, &pcm, 24_000)
            .with_context(|| format!("cannot write {}", path.display()))?;
    }

    // 3. Le tableau, pour la comparaison entre packs.
    let mut table = String::from("ttfa_ms\trtf\tduration_s\trms\ttext\n");
    table.push_str(&rows.join("\n"));
    table.push('\n');
    std::fs::write(args.out.join("measurements.tsv"), &table).ok();
    println!("peak rss: {:.0} Mo", peak_rss_mb());
    println!("wrote {} renders in {}", rows.len(), args.out.display());
    Ok(())
}
