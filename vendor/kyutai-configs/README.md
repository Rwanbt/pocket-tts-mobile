# Configs YAML Kyutai (`pocket_tts/config`)

Copie verbatim des configs de modèle du paquet Python
[kyutai-labs/pocket-tts](https://github.com/kyutai-labs/pocket-tts)
(branche `main`, dossier `pocket_tts/config/`).

Ces YAML décrivent l'architecture d'un modèle pocket-tts (flow LM + codec Mimi).
L'adaptateur `src/kyutai/config.rs` les traduit vers le schéma `config.json` de
`ptts` (`ptts::tts_model::TTSConfig`) ; le binaire `make_config` produit ce JSON.

## Pourquoi une copie locale

`kyutai-labs/pocket-tts/rust/build.rs` embarque ces YAML à la compilation, depuis
`../pocket_tts/config`. Le crate mobile n'embarque pas le paquet Python, donc les
YAML sont copiés ici et `build.rs` les compile dans `$OUT_DIR/embedded.rs`.

## Mise à jour

```sh
git clone --depth 1 https://github.com/kyutai-labs/pocket-tts
cp pocket-tts/pocket_tts/config/*.yaml <ce dossier>/
cp pocket-tts/LICENSE <ce dossier>/LICENSE
```

## Licence

MIT — voir `LICENSE`. Copyright Kyutai Labs.
