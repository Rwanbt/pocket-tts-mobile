//! Binaire de test pour Pocket TTS Mobile
//!
//! Ce binaire est utilisé pour tester le moteur Rust en local (x86_64)
//! avant le déploiement sur Android.
//!
//! # Utilisation
//!
//! ```bash
//! cargo run --release -- --text "Bonjour le monde" --output test.wav
//! ```

use anyhow::Result;
use clap::Parser;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use pocket_tts_mobile::ffi::{
    ptt_create, ptt_free, ptt_stream_end, ptt_stream_read, ptt_stream_start,
};
use std::ffi::CString;

/// Outil de test pour Pocket TTS Mobile
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Texte à synthétiser
    #[arg(short, long)]
    text: String,

    /// Chemin vers le pack modèle (ex: ./voice-models/pocket/fr-24l-q8)
    #[arg(short, long, default_value = "./voice-models/pocket/fr-24l-q8")]
    model_path: PathBuf,

    /// Fichier de sortie WAV
    #[arg(short, long, default_value = "output.wav")]
    output: PathBuf,

    /// Température (0.0 = déterministe)
    #[arg(short, long, default_value_t = 0.0)]
    temperature: f32,

    /// ID de la voix (0 = par défaut)
    #[arg(short, long, default_value_t = 0)]
    voice_id: i32,

    /// Verbose
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.verbose {
        println!("Pocket TTS Mobile - Test Binary");
        println!("Model path: {}", args.model_path.display());
        println!("Output: {}", args.output.display());
        println!("Text: {}", args.text);
    }

    // Convertir les chemins en CString
    let model_path_c = CString::new(args.model_path.to_str().unwrap_or_default())?;
    let text_c = CString::new(&args.text)?;

    // Créer une session TTS
    let handle = unsafe { ptt_create(model_path_c.as_ptr()) };
    if handle.is_null() {
        anyhow::bail!("Failed to create TTS session");
    }

    // Démarrer le stream
    let result =
        unsafe { ptt_stream_start(handle, text_c.as_ptr(), args.temperature, args.voice_id) };
    if result != 0 {
        anyhow::bail!("Failed to start stream");
    }

    // Lire le stream et écrire dans le fichier WAV
    let mut wav_file = File::create(&args.output)?;

    // Écrire l'en-tête WAV (24 kHz mono f32)
    // Note: Pour simplifier, on écrit en raw f32 pour l'instant
    // TODO: Ajouter l'en-tête WAV complet
    let sample_rate = 24000;
    let bits_per_sample = 32;
    let channels = 1;

    // Buffer temporaire pour les chunks
    const BUFFER_SIZE: usize = 1024;
    let mut buffer = vec![0.0f32; BUFFER_SIZE];

    let mut total_samples = 0usize;
    loop {
        let n = unsafe { ptt_stream_read(handle, buffer.as_mut_ptr(), BUFFER_SIZE) };
        if n <= 0 {
            break;
        }
        let n = n as usize;

        // Écrire les samples dans le fichier (raw f32 pour l'instant)
        for i in 0..n {
            wav_file.write_all(
                &buffer[i..i + 1]
                    .iter()
                    .flat_map(|x| x.to_le_bytes())
                    .collect::<Vec<u8>>(),
            )?;
        }
        total_samples += n;
    }

    // Terminer le stream
    unsafe { ptt_stream_end(handle) };

    // Libérer la session
    unsafe { ptt_free(handle) };

    if args.verbose {
        println!(
            "Generated {} samples ({} seconds at 24 kHz)",
            total_samples,
            total_samples as f32 / sample_rate as f32
        );
        println!("Output written to: {}", args.output.display());
    }

    Ok(())
}
