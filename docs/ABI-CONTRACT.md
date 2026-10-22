# Contrat ABI `ptt_*` — Pocket TTS Mobile ↔ Unifia

> Source de vérité : `packages/mobile/src-tauri/src/voice/pocket_tts.rs` (worktree `stt-parakeet`), qui charge
> `libpocket_tts.so` (PocketTTS.cpp, MIT) via `libloading`.
> La cdylib Rust doit exposer **exactement** ces symboles, pour que l'appelant Tauri ne change pas.

---

## 1. Symboles et signatures

```c
/* Crée un moteur à partir d'un pack. Renvoie NULL en cas d'échec. */
void* ptt_create(const char* models_dir,     /* dossier du pack : graphs + config + poids */
                 const char* voices_dir,     /* dossier des voix                      */
                 const char* tokenizer_path, /* fichier de tokenizer                  */
                 const char* precision,      /* "int8"                               */
                 float       temperature,     /* 0.3 en / 0.5 fr / 0.7 sinon          */
                 int         lsd_steps,       /* 1                                    */
                 int         num_threads);    /* 2                                    */

/* Libère le moteur. Exactement une fois, après la fin de tous ses streams. */
void   ptt_destroy(void* engine);

/* Préchauffe le moteur. Renvoie la durée en ms (f64). */
double ptt_warmup(void* engine);

/* Applique le front-end texte. Renvoie 0 si accepté, non-zéro si rejeté. */
int    ptt_set_text_options(void* engine,
                            int   eos_extra_frames,  /* -1 = devine (3 ou 5) ; fr = 8      */
                            int   pad_short_inputs,  /* 0/1 ; fr = 0                        */
                            int   remove_semicolons, /* 0/1 ; fr = 1                        */
                            int   max_chunk_tokens); /* 0 = 1 phrase par génération ; fr = 50*/

/* Démarre une synthèse. Renvoie NULL en cas d'échec. */
void*  ptt_stream_start(void* engine, const char* text, const char* voice);

/* Lecture bloquante. Le runtime ALLOQUE le tampon, l'appelant le libère. */
int    ptt_stream_read(void* stream, float** out_samples, int* out_length);

/* Arrête le stream et libère le contexte. */
void   ptt_stream_end(void* stream);

/* Libère un tampon renvoyé par ptt_stream_read. */
void   ptt_free_audio(float* samples);
```

---

## 2. Sémantique exacte (à respecter à la lettre)

| Point | Contrat | Piège |
|---|---|---|
| `ptt_stream_read` | **Alloue** le tampon (`float**` en sortie) | Un wrapper qui fournit le tampon à la place casse l'ABI : `ptt_free_audio` n'aurait rien à libérer |
| `ptt_stream_read` renvoie | `1` = chunk valide ; tout autre valeur = fin de stream ou erreur | Ce n'est **pas** le nombre de samples ; le nombre est dans `out_length` |
| `ptt_stream_end` | Prend **uniquement** le contexte de stream | Pas le handle moteur ; le moteur vit plus longtemps que les streams |
| `ptt_destroy` | Nom exact `ptt_destroy` | `ptt_free` n'existe pas dans l'ABI officielle |
| `ptt_create` | 7 arguments | Une version à 1 argument n'est pas interopérable |
| `ptt_stream_start` | Renvoie un `void*` de contexte | Ne renvoie pas un code d'erreur |
| `ptt_warmup` | Renvoie un `f64` en ms | Utile pour mesurer le TTFA et le coût de chargement |
| `ptt_set_text_options` | Renvoie 0 si accepté | L'appelant **détruit** le moteur si le retour n'est pas 0 |
| Erreurs | Aucun canal d'erreur : l'appelant déduit l'échec d'un pointeur nul ou d'un retour non-1 | Les erreurs doivent être **loguées** côté natif, sinon elles sont invisibles (le WebView ne remonte pas la console vers logcat) |
| Réentrance | Le moteur n'est pas réentrant : un stream à la fois | L'appelant termine tous les streams avant d'en démarrer un nouveau |

---

## 3. Chaîne d'appel côté Unifia

```
voice_pocket_prepare(language)
  └─ find_pack(roots, language)      PACK_FILES doivent tous être présents + voices/*.wav
  └─ ptt_create(models, voices, tokenizer, "int8", temperature, 1, 2)
  └─ ptt_set_text_options(handle, eos_extra_frames, pad, remove_semicolons, max_chunk)
  └─ ptt_warmup(handle)              → warmup_ms logué
  └─ (garde) ptt_destroy si set_text_options a échoué

voice_pocket_stream_start(text, voice?)
  └─ end_all_streams()               un seul stream à la fois
  └─ resolve_voice(pack, voice)      nom de fichier dans voices/
  └─ ptt_stream_start(handle, text, voice) → ctx

voice_pocket_stream_read(id)         [bloquant, dans spawn_blocking]
  └─ ptt_stream_read(ctx, &samples, &length)
  └─ status != 1 ou samples == NULL → encode_chunk(0, &[]) = fin
  └─ encode_chunk(24_000, chunk)     u32 LE rate || PCM16 LE mono
  └─ ptt_free_audio(samples)

voice_pocket_stream_end(id)
  └─ ptt_stream_end(ctx)
```

---

## 4. Ce qui change avec le runtime Rust (à traiter en Phase 3)

L'ABI **des fonctions** reste identique ; c'est le **contenu** des packs qui change.

| Élément | Pack ONNX actuel | Pack Rust cible | Conséquence |
|---|---|---|---|
| Graphes | 7 fichiers `.onnx` (`PACK_FILES`) | `config.json` + `model.safetensors` / `model.q8.gguf` | `PACK_FILES` doit être réécrit (constante dans `pocket_tts.rs`) |
| Tokenizer | `tokenizer.model` (SentencePiece) | `tokenizer.json` (HF) | `ptt_create` reçoit le nouveau chemin ; l'implémentation Rust n'a pas besoin de sentencepiece |
| Voix | `voices/*.wav` (prompt de conditionnement) | `embeddings/*.safetensors` | `pack_voices()` et `resolve_voice()` doivent filtrer `.safetensors` au lieu de `.wav` |
| Précision | `"int8"` | à définir (`"f32"`, `"q8"`…) | Chaîne passée par l'appelant ; l'ABI accepte n'importe quelle chaîne, mais l'appelant et la lib doivent être d'accord |
| Text options | 4 `int` via `ptt_set_text_options` | **identique** | L'adaptateur Kyutai implémente ces 4 options : c'est le point d'accroche du front-end FR |
| Température | 0.3 en / 0.5 fr / 0.7 autres | identique | Les valeurs mesurées (WER 0.048 à 0.5) restent valables |
| Échantillonnage | 24 kHz mono f32 → PCM16 | identique | `POCKET_SAMPLE_RATE_HZ = 24_000` inchangé |

**Conséquence clé** : `ptt_set_text_options` est *exactement* le crochet qui permet de porter les réglages FR
(`frames_after_eos = 8`, `remove_semicolons`, `max_chunk_tokens = 50`) sans toucher au front-end Kyutai en dur.
Une cdylib qui ignorerait ce appel réintroduirait le bug « mot final avalé ».

---

## 5. Tests d'interopérabilité à écrire (Phase 3)

1. Résolution des 8 symboles via `libloading` — l'erreur doit être `POCKET_UNAVAILABLE: resolve symbol: …`.
2. `ptt_create` avec un pack incomplet → `NULL`, jamais de panique.
3. `ptt_set_text_options` qui renvoie non-0 → le moteur est détruit, pas de fuite.
4. `ptt_stream_read` qui renvoie 0 avec `samples == NULL` → chunk vide (fin), pas de `free_audio` inutile.
5. `ptt_free_audio` sur un tampon alloué par `ptt_stream_read` (et **seulement** sur celui-là).
6. `ptt_stream_end` appelé deux fois → idempotent ou erreur explicite, jamais de double free.
7. Un stream annulé pendant un `read` en vol → pas de use-after-free (le mutex de l'appelant sérialise read/end).
