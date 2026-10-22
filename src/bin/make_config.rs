//! Genere le `config.json` d'un pack modele ptts a partir d'une config YAML Kyutai.
//!
//! Approche A du spike : on reutilise l'adaptateur Kyutai (`src/kyutai/config.rs`),
//! qui traduit `pocket_tts/config/<langue>.yaml` vers le schema serde de
//! `ptts::tts_model::TTSConfig`. Le JSON produit est le `config.json` attendu par
//! `ptts::checkpoint::Checkpoint::locate`.
//!
//! Usage :
//!   cargo run --bin make_config -- --config french_24l --out D:/App/unifia/models/fr/config.json

use anyhow::{Context, Result, bail};
use pocket_tts_mobile::kyutai::config::Config;
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut config_name = "french_24l".to_string();
    let mut out = PathBuf::from("config.json");
    let mut steps: usize = pocket_tts_mobile::kyutai::defaults::DEFAULT_SAMPLER_DECODE_STEPS;
    let mut eos_threshold: Option<f32> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || -> Result<String> {
            args.next().with_context(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--config" => config_name = value()?,
            "--out" => out = PathBuf::from(value()?),
            "--steps" => steps = value()?.parse().context("--steps must be a number")?,
            "--eos-threshold" => {
                eos_threshold = Some(value()?.parse().context("--eos-threshold must be a number")?)
            }
            "--list" => {
                for name in pocket_tts_mobile::kyutai::available_configs() {
                    println!("{name}");
                }
                return Ok(());
            }
            other => bail!("unknown argument {other:?}"),
        }
    }

    let yaml = pocket_tts_mobile::kyutai::config_yaml(&config_name).with_context(|| {
        format!(
            "no embedded config named {config_name:?}; available: {:?}",
            pocket_tts_mobile::kyutai::available_configs()
        )
    })?;

    let config = Config::parse(yaml).with_context(|| format!("invalid Kyutai config {config_name}"))?;
    let eos_threshold = eos_threshold.unwrap_or(config.default_eos_threshold);
    let ptts_config = config
        .to_ptts(steps, eos_threshold)
        .with_context(|| format!("cannot translate {config_name} to a ptts config"))?;

    let json = serde_json::to_string_pretty(&ptts_config).context("cannot serialize the config")?;
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("cannot create {parent:?}"))?;
    }
    std::fs::write(&out, format!("{json}\n"))
        .with_context(|| format!("cannot write {}", out.display()))?;

    println!("wrote {}", out.display());
    println!("{json}");
    Ok(())
}
