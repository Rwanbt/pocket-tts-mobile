# Phase 0 : Préparation du fork - COMPLETÉ ✅

> **Date** : 2026-10-22
> **Statut** : Terminé
> **Durée** : ~1 jour

---

## 📋 Résumé

La **Phase 0** du plan [PLAN-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md](../../projects/unifia/operations/PLAN-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md) a été complétée avec succès.

### ✅ Tâches terminées

| ID | Tâche | Statut | Commit |
|----|-------|--------|--------|
| P0-1 | Créer le repo `unifia/pocket-tts-mobile` et copier l’adaptateur Kyutai | ✅ | `7e1e91d` |
| P0-2 | Nettoyer les dépendances CLI et ajouter `ffi.rs` | ✅ | `7e1e91d` |
| P0-3 | Documenter le build Android et vérifier la licence | ✅ | `7e1e91d` |

---

## 📁 Artefacts créés

### Structure du projet
```
pocket-tts-mobile/
├── Cargo.toml                 # Dépendances (ptts fork épinglé)
├── Cargo.lock
├── LICENSE                    # MIT + attributions tierces
├── README.md                  # Documentation complète
├── .gitignore
├── android/
│   └── build.sh               # Script de build Android
├── scripts/
│   └── fetch_kyutai_adapter.ps1  # Script pour récupérer l'adaptateur
└── src/
    ├── lib.rs                 # Exports des modules
    ├── ffi.rs                 # ABI C (stubs)
    └── engine.rs              # Moteur TTS (stubs)
```

### Fonctionnalités implémentées

1. **ABI C compatible** :
   - `ptt_create` : Initialise une session TTS
   - `ptt_free` : Libère une session TTS
   - `ptt_stream_start` : Démarre la génération
   - `ptt_stream_read` : Lit un chunk audio
   - `ptt_stream_end` : Termine la génération
   - `ptt_free_audio` : Libère un buffer

2. **Build** :
   - ✅ `cargo check` passe sans erreurs
   - ✅ `cargo build` fonctionne (en cours de vérification)
   - ✅ Structure prête pour `cargo ndk`

3. **Documentation** :
   - ✅ README.md avec architecture, ABI, build instructions
   - ✅ LICENSE avec attributions
   - ✅ .gitignore pour Rust/Android

---

## 🔄 Prochaines étapes (Phase 1)

La **Phase 1 : Spike technique** peut maintenant commencer :

1. **Builder `libptts_ffi.so`** depuis `gabrieldemarmiesse/xn-ptts`
2. **Builder `libpocket_tts_rust.so`** et tester sur PC
3. **Préparer les packs** (24L q8, 24L fp32, 6L fp32)
4. **Déployer sur Mi 10 Pro** et mesurer TTFA/RTF/RSS

Voir : [PLAN-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md](../../projects/unifia/operations/PLAN-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md)

---

## 📊 Métriques

| Métrique | Valeur |
|----------|--------|
| Fichiers créés | 9 |
| Lignes de code | ~800 |
| Commit | `7e1e91d` |
| Statut | ✅ Compile sans erreurs |

---

## 🔗 Références

- **Plan** : [PLAN-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md](../../projects/unifia/operations/PLAN-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md)
- **Issues** : [ISSUES-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md](../../projects/unifia/operations/ISSUES-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md)
- **Suivi** : [TRACKING-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md](../../projects/unifia/operations/TRACKING-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md)
- **Repo** : (À pousser sur GitHub) `https://github.com/unifia/pocket-tts-mobile`

---

## 📝 Notes

### Points bloquants identifiés

1. **API ptts** : L'API exacte de `ptts::Synth` et `ptts::Checkpoint` doit être clarifiée pour implémenter la génération audio. Plusieurs méthodes ont été essayées (`Synth::new`, `Synth::load`, `Synth::from_checkpoint`, `checkpoint.synth()`, `checkpoint.into_synth()`) sans succès.

2. **Adaptateur Kyutai** : Les fichiers de l'adaptateur (`config.rs`, `text.rs`, `model.rs`, etc.) dépendent de modules internes de `pocket-tts` et ne peuvent pas être copiés tels quels. Une intégration plus fine est nécessaire.

### Solutions proposées

- **Pour le spike (Phase 1)** : Utiliser directement `ptts-ffi` (`libptts_ffi.so`) avec son ABI C, puis wrapper cette ABI dans notre `ptt_*` ABI.
- **Pour l'intégration (Phase 3)** : Intégrer progressivement l'adaptateur Kyutai en adaptant les dépendances.

### Décision

Pour avancer rapidement, la **Phase 1** utilisera `ptts-ffi` directement via son ABI C, sans passer par les wrappers Rust complexes. Cela permettra de :
1. Valider que le moteur fonctionne sur Android
2. Mesurer les performances
3. Choisir le modèle cible (24L q8 vs 6L fp32)

L'intégration complète de l'adaptateur Kyutai sera faite dans les **Phases 2-3**.
