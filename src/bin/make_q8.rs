//! Quantifie un pack modele en q8_0 (GGUF), pour le pack Android.
//!
//! Les poids de la branche flow-LM (les lineaires du transformer) sont quantifies
//! en q8_0 ; le reste (flow net, Mimi, embeddings) est garde tel quel. C'est le
//! meme procede que l'exemple `quantize` de `ptts`.
//!
//! Usage :
//!   cargo run --release --bin make_q8 -- <model.safetensors> <model.q8.gguf>
//!
//! Le fichier de poids doit avoir ete prepare par `scripts/prepare_pack.py`
//! (le tenseur `flow_lm.bos_before_voice` y est renomme en
//! `flow_lm.condition_provider.conditioners.speaker_wavs.sink`, le nom que
//! `ptts` attend) : `ptts::loader::remap_key` ne fait pas ce renommage.

use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use xn::quantized::{GgmlDType, QStorage, QTensor, gguf_file};
use xn::{CPU, TypedTensor, safetensors};

/// Poids des lineares du transformer du flow-LM, les seuls quantifies.
const QUANT_SUFFIXES: &[&str] =
    &["linear1.weight", "linear2.weight", "self_attn.in_proj.weight", "self_attn.out_proj.weight"];

fn should_quantize(name: &str) -> bool {
    name.starts_with("flow_lm.transformer.layers.")
        && QUANT_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

fn tensor_to_f32(tensor: &TypedTensor<xn::CpuDevice>) -> Result<Vec<f32>> {
    let values = match tensor {
        TypedTensor::F32(t) => t.to_vec()?,
        TypedTensor::F16(t) => t.to_vec()?.into_iter().map(|v| v.to_f32()).collect(),
        TypedTensor::BF16(t) => t.to_vec()?.into_iter().map(|v| v.to_f32()).collect(),
        TypedTensor::I64(_) | TypedTensor::U8(_) => bail!("cannot quantize a non-float tensor"),
    };
    Ok(values)
}

fn as_is_qtensor(tensor: &TypedTensor<xn::CpuDevice>) -> Result<QTensor> {
    let shape = tensor.shape().clone();
    let qtensor = match tensor {
        TypedTensor::F32(t) => QTensor::new(QStorage::Cpu(Box::new(t.to_vec()?)), shape)?,
        TypedTensor::F16(t) => QTensor::new(QStorage::Cpu(Box::new(t.to_vec()?)), shape)?,
        TypedTensor::BF16(t) => QTensor::new(QStorage::Cpu(Box::new(t.to_vec()?)), shape)?,
        TypedTensor::I64(_) => bail!("I64 tensors cannot be stored in GGUF"),
        TypedTensor::U8(_) => bail!("U8 tensors cannot be stored in GGUF"),
    };
    Ok(qtensor)
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let input = PathBuf::from(args.next().context("usage: make_q8 <in.safetensors> <out.gguf>")?);
    let output =
        PathBuf::from(args.next().context("usage: make_q8 <in.safetensors> <out.gguf>")?);
    let quant = match args.next().as_deref() {
        None | Some("q8") | Some("q8_0") => GgmlDType::Q8_0,
        Some("q6k") => GgmlDType::Q6K,
        Some(other) => bail!("unsupported quantization {other:?}"),
    };

    println!("loading {}", input.display());
    let tensors = safetensors::load_from_file(&input, &CPU)
        .with_context(|| format!("cannot read {}", input.display()))?;

    let mut names: Vec<&String> = tensors.keys().collect();
    names.sort();

    let mut qtensors: Vec<(String, QTensor)> = Vec::with_capacity(names.len());
    let mut quantized_bytes = 0usize;
    for name in names {
        let tensor = &tensors[name];
        let qtensor = if should_quantize(name) {
            let data = tensor_to_f32(tensor)?;
            quantized_bytes += data.len() * 4;
            QTensor::quantize_f32(&data, tensor.shape(), quant)?
        } else {
            as_is_qtensor(tensor)?
        };
        qtensors.push((name.clone(), qtensor));
    }
    println!("quantized the transformer linears of {} tensors", qtensors.len());

    let refs: Vec<(&str, &QTensor)> = qtensors.iter().map(|(n, t)| (n.as_str(), t)).collect();
    println!("writing {}", output.display());
    let file = std::fs::File::create(&output)
        .with_context(|| format!("cannot create {}", output.display()))?;
    let mut writer = std::io::BufWriter::new(file);
    gguf_file::write(&mut writer, &[], &refs)?;
    drop(writer);

    let size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
    println!("wrote {} ({size} bytes)", output.display());
    Ok(())
}
