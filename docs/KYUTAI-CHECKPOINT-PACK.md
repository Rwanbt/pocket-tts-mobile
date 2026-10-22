# Pack modèle Kyutai pour `ptts` — format, conversions et pièges

Référence factuelle pour préparer le pack que la cdylib Android charge.
Tout ce qui suit a été **vérifié par exécution** sur le checkpoint
`kyutai/pocket-tts-without-voice-cloning` rev `1e08e6a2`, langue `french_24l`.

## 1. Ce que publie Kyutai, et ce que `ptts` attend

| Fichier publié par Kyutai                | Ce que `ptts` attend                                  |
| ---------------------------------------- | ----------------------------------------------------- |
| `languages/french_24l/model.safetensors` | `model.safetensors` (safetensors **bf16**, 358 tenseurs) |
| `languages/french_24l/tokenizer.json`    | `tokenizer.json` (identical, 246 403 o)                |
| `languages/french_24l/embeddings/*.safetensors` | **aucun equivalent** — voir §3                |
| — (absent)                               | `config.json` — voir §2                                |

Les poids sont en **bf16**, pas en fp32 : 641 Mo pour 24 couches.
Le fichier de 6 couches (`languages/french`) fait 209 Mo.

## 2. `config.json` : la config YAML suffit

`ptts` lit un `config.json` suivant le schéma serde de
`ptts::tts_model::TTSConfig`. Kyutai publie des YAML (`pocket_tts/config/*.yaml`),
que l'adaptateur `src/kyutai/config.rs` traduit :

```sh
cargo run --release --bin make_config -- --config french_24l --out <pack>/config.json
```

La traduction est fidèle : `Config::to_ptts` produit la config que le binaire
officiel Kyutai passe lui-meme a `ptts::tts_model::TTSModel::load`.
Les seuls parametres a choisir sont `lsd_decode_steps` (1 par defaut) et
`eos_threshold` (-4.0 par defaut, `DEFAULT_EOS_THRESHOLD` de Kyutai).

## 3. Piège 1 : le tenseur du « sink » est renomme

Kyutai publie le tenseur `flow_lm.bos_before_voice`.
`ptts` le cherche sous le nom du sink de la condition `speaker_wavs` :

```
flow_lm.condition_provider.conditioners.speaker_wavs.sink
```

`ptts::loader::remap_key` (rev `7efa269`) **ne fait pas** ce renommage ;
seul `kyutai-labs/pocket-tts/rust/src/weights.rs` le fait. Sans correction,
le chargement echoue :

```
Error: cannot build the synth
Caused by:
    tensor 'flow_lm.condition_provider.conditioners.speaker_wavs.sink' not found
```

Correction : renommer la cle dans le fichier de poids, hors ligne.
`scripts/prepare_pack.py` le fait (le fichier est reecrit, l'en-tete change de
taille, les `data_offsets` restent relatifs au debut des donnees).

```sh
python scripts/prepare_pack.py <source>/model.safetensors <pack>/model.safetensors
```

## 4. Piège 2 : les voix publiees ne sont pas des voix `ptts`

`embeddings/estelle.safetensors` (30 Mo) n'est pas une embedde voix : ce sont
des **etats de cache KV** du flow LM, au format Kyutai :

```
transformer.layers.<n>.self_attn/cache   F32  [2, 1, 154, 16, 64]
transformer.layers.<n>.self_attn/offset  I64  [1]
transformer.layers.<n>.self_attn/pad     I64  [1]
```

`ptts` attend soit une embedde `emb` `[T, dim]`, soit des latents
`speaker_wavs` `[C, T]`. Il rejette le fichier :

```
voice "estelle" REJECTED: voice tensor `transformer.layers.5.self_attn/cache` in
voice file .../embeddings/estelle.safetensors has shape [2, 1, 154, 16, 64],
expected two or three dimensions
```

Et l'echec est **silencieux** : `Checkpoint::register_voices` avale l'erreur dans
un `tracing::warn!`. Le modele se charge, mais sans voix, et l'audio est genere
sans condition vocale.

**Conclusion : le pack mobile ne doit pas contenir `embeddings/`.**
Le binaire officiel Kyutai, lui, les lit (il implemente ce format).

## 5. Le modele public ne sait pas cloner une voix

`kyutai/pocket-tts-without-voice-cloning` refuse le clonage vocal :

```
ERROR: We could not download the weights for the model with voice cloning, but
you're trying to use voice cloning. Without voice cloning, you can use our
catalog of voices ["cosette", "marius", ...].
```

C'est coherent avec le nom du depot. Consequence pour `ptts` :
`Synth::add_voice_from_pcm` accepte de cloner un audio, mais le resultat est du
bruit de parole (verifie : Whisper transcrit `PAcpAc` sur un WAV genere ainsi).
`ptts` ne peut pas signaler l'erreur, le modele ne le sait pas.

La seule voix utilisable avec ce modele est l'etat de cache KV de Kyutai, que
`ptts::Synth` ne sait pas lire.

## 6. Genere sans condition vocale : ca marche

`ptts::Synth` sans voix utilise le sink appris (`bos_before_voice`) et genere
du francais correct. Verifie avec Whisper (`small`, `--language fr`) :

| Texte                                                | Transcription Whisper                                                                 |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `Bonjour le monde.`                                  | `Bonjour le monde.`                                                                    |
| `Bonjour le monde. La dictée vocale est maintenant disponible en français.` | `Bonjour le monde, la dictée vocale est maintenant disponible en français.` |

## 7. Mesures (PC, 16 threads logiques, Windows)

|                          | f32 / bf16 safetensors | q8_0 GGUF                  |
| ------------------------ | ----------------------- | --------------------------- |
| Poids                    | 641 Mo                  | 371 Mo (-42 %)              |
| Pack complet             | 641,3 Mo                | 371,3 Mo                   |
| Chargement (a chaud)     | 0,93 - 1,03 s           | 0,43 - 0,57 s              |
| Pic RSS                  | 1 862 Mo                | 850 Mo (-54 %)              |
| RTF (4,2 s d'audio)      | 0,94 (machine libre)    | 1,75 (machine chargee)      |

Le q8_0 divise par deux le temps de chargement et la memoire, mais la
generation est plus lente (~1,5x) sur ce CPU : la dequantification n'est pas
gratuite. A mesurer sur un appareil Android avant de choisir.

Les valeurs de RTF dependent fortement de la charge de la machine (mesures
prises en presence d'autres sessions de compilation) ; le chiffre `0,94` est la
mesure la plus favorable, sur une machine libre.

## 8. Comparaison avec le binaire officiel

Le binaire officiel (`kyutai-labs/pocket-tts`, `rust/`, meme revision de `ptts`
que nous) a ete compile et utilise comme reference :

```sh
pocket-tts generate --language french_24l --voice estelle \
  --text "Bonjour le monde." --output-path out.wav
```

Avec les **memes** poids (`1e08e6a2`, apres repli de `weights_path` sur
`weights_path_without_voice_cloning`) et la voix `estelle` officielle, il
produit le meme texte que `ptts::Synth` sans condition vocale, en RTF 0,40.

## 9. Commandes completes du spike

```sh
cd pocket-tts-mobile

# 1. config.json (approche A : adaptateur Kyutai)
cargo run --release --bin make_config -- \
  --config french_24l --out D:/App/unifia/models/fr/config.json

# 2. poids : renommer le tenseur du sink
python scripts/prepare_pack.py \
  D:/App/unifia/models/fr/model.kyutai.safetensors \
  D:/App/unifia/models/fr/model.safetensors

# 3. (optionnel) version q8_0
cargo run --release --bin make_q8 -- \
  D:/App/unifia/models/fr/model.safetensors \
  D:/App/unifia/models/fr/model.q8.gguf

# 4. WAV
cargo run --release --bin speak -- \
  --pack D:/App/unifia/models/fr --quant f32 \
  --text "Bonjour le monde. La dictée vocale est maintenant disponible en français." \
  --out D:/App/unifia/models/fr/out.wav

# 5. verification
python scripts/analyze_wav.py D:/App/unifia/models/fr/out.wav
whisper D:/App/unifia/models/fr/out.wav --model small --language fr
```
