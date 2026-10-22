//! Le tokenizer du modele.
//!
//! Repris de <https://github.com/kyutai-labs/pocket-tts> (`rust/src/tokenizer.rs`), MIT.
//! Voir `vendor/kyutai-configs/LICENSE`.
//!
//! # Une seule forme de tokenizer
//!
//! L'original supporte deux formes : un modele SentencePiece (`.model`) et un
//! tokenizer Hugging Face (`tokenizer.json`). Ici, **seule la forme JSON est
//! conservee** :
//!
//! - le pack mobile contient `tokenizer.json`, que Kyutai publie pour chaque
//!   langue ;
//! - SentencePiece est un binding C++ qui se compile avec `cmake`, ce qui
//!   weight-lourd et fragile a cross-compiler pour Android, pour une forme que
//!   le pack n'utilise pas.
//!
//! Une config ou un pack qui demanderait la forme SentencePiece echoue
//! explicitement, plutot que de tenter une compilation C++ sur Android.

use anyhow::{bail, Context, Result};
use std::sync::Arc;

pub enum Tokenizer {
    Json(Box<tokenizers::Tokenizer>),
}

impl Tokenizer {
    /// Charge le tokenizer d'un pack.
    ///
    /// `kind` vient de la config (`tokenizers`, `sentencepiece`, ...) ;
    /// `tokenizer_path` est le chemin du fichier, local dans le pack.
    pub fn load(n_bins: usize, tokenizer_path: &str, kind: &str) -> Result<Self> {
        let is_json = kind == "tokenizers"
            || kind == "json"
            || tokenizer_path.split('@').next().unwrap().ends_with(".json");
        if !is_json {
            bail!(
                "this build only supports the Hugging Face tokenizer (tokenizer.json), \
                 the pack asks for {kind:?} at {tokenizer_path:?}"
            );
        }
        let local_path = crate::kyutai::download::download_if_necessary(tokenizer_path)?;
        log::info!("Loading tokenizers json from {tokenizer_path}");
        let tokenizer = tokenizers::Tokenizer::from_file(&local_path)
            .map_err(|e| anyhow::anyhow!("{e}"))
            .with_context(|| format!("cannot load {}", local_path.display()))?;
        let vocab_size = tokenizer.get_vocab_size(true);
        if vocab_size != n_bins {
            bail!("tokenizer has vocab size={vocab_size} but nbins={n_bins} was specified")
        }
        Ok(Self::Json(Box::new(tokenizer)))
    }

    pub fn encode(&self, text: &str) -> Result<Vec<u32>> {
        match self {
            // `add_special_tokens` as in Python's `Tokenizer.encode` default.
            Self::Json(tok) => Ok(tok
                .encode(text, true)
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .get_ids()
                .to_vec()),
        }
    }

    pub fn decode(&self, ids: &[u32]) -> Result<String> {
        match self {
            Self::Json(tok) => tok.decode(ids, true).map_err(|e| anyhow::anyhow!("{e}")),
        }
    }
}

/// The tokenizer as xn-ptts's flow LM holds it, shared with the text chunking.
pub struct SharedTokenizer(pub Arc<Tokenizer>);

impl ptts::Tokenizer for SharedTokenizer {
    fn encode(&self, text: &str) -> xn::Result<Vec<u32>> {
        self.0
            .encode(text)
            .map_err(|e| xn::Error::msg(format!("{e:#}")))
    }

    fn decode(&self, tokens: &[u32]) -> xn::Result<String> {
        self.0
            .decode(tokens)
            .map_err(|e| xn::Error::msg(format!("{e:#}")))
    }
}
