# Issues Template pour le portage Rust Pocket TTS

> **Épic** : [Épic] Portage Rust Pocket TTS sur Android
> **Repo** : Rwanbt/unifia
> **Plan** : [PLAN-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md](../../projects/unifia/operations/PLAN-Portage-Rust-Pocket-TTS-Mobile-2026-10-22.md)

---

## 📌 Épic

### `[Épic] Portage Rust Pocket TTS sur Android`

**Description** :
Porter le runtime Rust officiel de Pocket TTS (Kyutai) sur Android pour remplacer progressivement le moteur C++/ONNX actuel. Ce travail inclut un comparatif entre les modèles 24L int8 (actuel) et 6L/24L fp32/q8 (Rust) pour déterminer le meilleur compromis qualité/performance/mémoire.

**Plan détaillé** : [Lien vers le plan dans le vault]()

**Priorité** : `high`
**Assigné** : @erwanbarat
**Labels** : `enhancement`, `voice`, `rust`, `android`, `mobile`, `epic`
**Statut** : `To Do`

---

## 📋 Issues par Phase

---

### 🟡 Phase 0 : Préparation du fork ✅ COMPLETÉ

#### ✅ `[P0-1] Créer le repo unifia/pocket-tts-mobile et copier l’adaptateur Kyutai`
- **Statut** : ✅ Terminé
- **Commit** : `7e1e91d`
- **Livrable** : Repo créé avec structure de base

#### ✅ `[P0-2] Nettoyer les dépendances CLI et ajouter ffi.rs`
- **Statut** : ✅ Terminé
- **Commit** : `7e1e91d`
- **Livrable** : ABI C implémentée (stubs)

#### ✅ `[P0-3] Documenter le build Android et vérifier la licence`
- **Statut** : ✅ Terminé
- **Commit** : `7e1e91d`
- **Livrable** : README.md, LICENSE, build.sh

---

### 🟡 Phase 1 : Spike technique (1–3 jours)

#### ⏳ `[P1-1] Builder libptts_ffi.so depuis xn-ptts et tester sur PC`
**Description** :
- Cloner `gabrieldemarmiesse/xn-ptts` (branch `pocket-tts-small-models`, rev `7efa269`)
- Builder `libptts_ffi.so` pour x86_64 avec `cargo build --release`
- Tester avec un checkpoint français

**Critères d'acceptation** :
- [ ] `libptts_ffi.so` généré pour x86_64
- [ ] WAV généré et valide (24 kHz mono f32)
- [ ] Pas d'erreur de runtime

**Labels** : `phase-1`, `backend`, `rust`, `spike`, `testing`
**Assigné** : @erwanbarat
**Priorité** : `high`
**Dépendances** : Phase 0 ✅
**Estimation** : 4h

---

#### ⏳ `[P1-2] Builder libpocket_tts_rust.so et tester sur PC`
**Description** :
- Builder le shim Unifia (`pocket-tts-mobile`) pour x86_64
- Tester avec un checkpoint local
- Vérifier que l'ABI `ptt_*` fonctionne

**Critères d'acceptation** :
- [ ] `libpocket_tts_rust.so` généré
- [ ] Appels `ptt_create`/`ptt_stream_start`/`ptt_stream_read`/`ptt_stream_end` fonctionnels
- [ ] WAV généré

**Labels** : `phase-1`, `backend`, `rust`, `spike`, `testing`
**Assigné** : @erwanbarat
**Priorité** : `high`
**Dépendances** : #P1-1
**Estimation** : 4h

---

#### ⏳ `[P1-3] Préparer les packs pour 24L q8, 24L fp32, 6L fp32`
**Description** :
- Télécharger les checkpoints depuis HF
- Générer `config.json` depuis `french_24l.yaml`
- Convertir `french_24l/model.safetensors` en `model.q8.gguf`
- Structurer les packs

**Critères d'acceptation** :
- [ ] Packs `fr-24l-q8`, `fr-24l-fp32`, `fr-6l-fp32` prêts
- [ ] `config.json` valide pour chaque pack

**Labels** : `phase-1`, `backend`, `data`, `spike`
**Assigné** : @erwanbarat
**Priorité** : `high`
**Dépendances** : #P1-2
**Estimation** : 4h

---

#### ⏳ `[P1-4] Déployer sur Mi 10 Pro et mesurer TTFA/RTF/RSS`
**Description** :
- Pousser `libpocket_tts_rust.so` et les packs sur le Mi 10 Pro
- Appeler le moteur via JNI
- Mesurer TTFA, RTF, RSS pour chaque modèle

**Critères d'acceptation** :
- [ ] WAV généré sur le Mi 10 Pro pour chaque pack
- [ ] TTFA < 500 ms pour au moins un pack
- [ ] RTF ≥ 2.0 pour au moins un pack
- [ ] RSS < 1 Go pour au moins un pack
- [ ] Rapport de spike généré

**Labels** : `phase-1`, `backend`, `android`, `benchmark`, `spike`
**Assigné** : @erwanbarat
**Priorité** : `high`
**Dépendances** : #P1-3
**Estimation** : 8h

---

### 🟡 Phase 2 : Comparatif complet (1 semaine)

#### ⏳ `[P2-1] Définir le jeu de test FR et mesurer WER`
**Description** :
- Créer un jeu de test de 10 phrases françaises
- Générer les WAV pour chaque modèle
- Mesurer le WER via Parakeet STT

**Critères d'acceptation** :
- [ ] Jeu de test défini (`tests/fr_benchmark.json`)
- [ ] WER mesuré pour chaque modèle
- [ ] WER ≤ 0.05 pour au moins un modèle Rust

**Labels** : `phase-2`, `testing`, `quality`, `benchmark`
**Assigné** : @erwanbarat
**Priorité** : `high`
**Dépendances** : Phase 1
**Estimation** : 4h

---

#### ⏳ `[P2-2] Test d’écoute manuelle et rapport subjectif`
**Description** :
- Écouter les WAV générés
- Vérifier les mots finaux, la qualité perçue
- Documenter les observations

**Critères d'acceptation** :
- [ ] Rapport d'écoute documenté
- [ ] Aucun mot final coupé pour le modèle retenu

**Labels** : `phase-2`, `testing`, `quality`, `ux`
**Assigné** : @erwanbarat
**Priorité** : `medium`
**Dépendances** : #P2-1
**Estimation** : 2h

---

#### ⏳ `[P2-3] Décider du modèle cible (24L q8 vs 6L fp32)`
**Description** :
- Comparer TTFA, RTF, RSS, WER, qualité subjective
- Décider du modèle à intégrer

**Critères d'acceptation** :
- [ ] Décision claire et justifiée
- [ ] Modèle cible documenté

**Labels** : `phase-2`, `decision`, `quality`
**Assigné** : @erwanbarat
**Priorité** : `high`
**Dépendances** : #P2-2
**Estimation** : 2h

---

### 🟡 Phase 3 : Intégration (1–2 semaines)

#### ⏳ `[P3-1] Intégrer libpocket_tts_rust.so dans Unifia`
**Description** :
- Remplacer `libpocket_tts.so` par `libpocket_tts_rust.so`
- Mettre à jour `pack_roots()` et `PACK_FILES`
- Vérifier la compatibilité ABI

**Critères d'acceptation** :
- [ ] `libpocket_tts_rust.so` intégré
- [ ] Pack FR chargé et utilisé sans erreur
- [ ] `cargo test` passe

**Labels** : `phase-3`, `backend`, `integration`, `android`
**Assigné** : @erwanbarat
**Priorité** : `high`
**Dépendances** : Phase 2
**Estimation** : 8h

---

#### ⏳ `[P3-2] Adapter les logs pour logcat`
**Description** :
- Configurer `tracing` pour utiliser `android_logger`
- Ajouter des logs pour les fonctions `ptt_*`

**Critères d'acceptation** :
- [ ] Logs visibles dans `logcat`
- [ ] Niveau de log configurable

**Labels** : `phase-3`, `backend`, `android`, `logging`
**Assigné** : @erwanbarat
**Priorité** : `medium`
**Dépendances** : #P3-1
**Estimation** : 4h

---

#### ⏳ `[P3-3] Mettre à jour la CI Android pour le build Rust`
**Description** :
- Ajouter un job `android-rust-tts` dans `.github/workflows/android.yml`
- Vérifier que la CI passe

**Critères d'acceptation** :
- [ ] Job CI ajouté et fonctionnel
- [ ] Build Rust intégré dans le pipeline Android

**Labels** : `phase-3`, `ci`, `android`, `rust`
**Assigné** : @erwanbarat
**Priorité** : `high`
**Dépendances** : #P3-1
**Estimation** : 4h

---

### 🟡 Phase 4 : Qualification (2–4 semaines)

#### ⏳ `[P4-1] Passer le gate FR (WER ≤ 0.05, RTF ≥ 2.0)`
**Description** :
- Exécuter le gate FR existant avec le nouveau moteur
- Vérifier WER ≤ 0.05, RTF ≥ 2.0

**Critères d'acceptation** :
- [ ] Gate FR passé
- [ ] Résultats documentés

**Labels** : `phase-4`, `testing`, `quality`, `gate`
**Assigné** : @erwanbarat
**Priorité** : `high`
**Dépendances** : Phase 3
**Estimation** : 8h

---

#### ⏳ `[P4-2] Vérifier les 6 gestes de validation`
**Description** :
- Tester les 6 gestes de validation physiques sur le Mi 10 Pro
- Vérifier que chaque geste fonctionne

**Critères d'acceptation** :
- [ ] Tous les gestes validés
- [ ] Aucun régression

**Labels** : `phase-4`, `testing`, `ux`, `validation`
**Assigné** : @erwanbarat
**Priorité** : `high`
**Dépendances** : #P4-1
**Estimation** : 4h

---

#### ⏳ `[P4-3] Tester les autres langues (EN, ES, DE, PT)`
**Description** :
- Tester chaque langue avec son pack correspondant
- Vérifier WER ≤ 0.05, RTF ≥ 2.0

**Critères d'acceptation** :
- [ ] Toutes les langues testées passent les gates

**Labels** : `phase-4`, `testing`, `quality`, `i18n`
**Assigné** : @erwanbarat
**Priorité** : `medium`
**Dépendances** : #P4-2
**Estimation** : 8h

---

#### ⏳ `[P4-4] Mettre à jour ADR-059 et ajouter PROVENANCE.md`
**Description** :
- Mettre à jour ADR-059 avec la décision finale
- Ajouter PROVENANCE.md pour les licences

**Critères d'acceptation** :
- [ ] ADR-059 mis à jour
- [ ] PROVENANCE.md ajouté

**Labels** : `phase-4`, `docs`, `legal`, `architecture`
**Assigné** : @erwanbarat
**Priorité** : `medium`
**Dépendances** : #P4-3
**Estimation** : 4h

---

### 🟡 Phase 5 : Upstream (optionnel, parallèle)

#### ⏳ `[P5-1] PR vers xn-ptts : exposer frames_after_eos dans le FFI`
**Description** :
- Proposer une PR à `gradium-ai/xn-ptts`
- Ajouter `frames_after_eos` dans `ptts.h` et `ptts_speak`

**Critères d'acceptation** :
- [ ] PR ouverte et documentée
- [ ] Discussions avec les mainteneurs

**Labels** : `phase-5`, `upstream`, `enhancement`, `ffi`
**Assigné** : @erwanbarat
**Priorité** : `low`
**Dépendances** : Aucune (parallèle)
**Estimation** : 4h

---

#### ⏳ `[P5-2] PR vers pocket-tts : ajouter le support Android officiel`
**Description** :
- Proposer une PR à `kyutai-labs/pocket-tts`
- Ajouter une feature `android` ou un crate `pocket-tts-ffi`

**Critères d'acceptation** :
- [ ] PR ouverte et documentée
- [ ] Discussions avec les mainteneurs

**Labels** : `phase-5`, `upstream`, `enhancement`, `android`
**Assigné** : @erwanbarat
**Priorité** : `low`
**Dépendances** : #P5-1
**Estimation** : 4h

---

## 📊 Résumé

| Phase | Issues | Statut |
|-------|--------|--------|
| Phase 0 | 3 | ✅ **Complété** |
| Phase 1 | 4 | ⏳ À faire |
| Phase 2 | 3 | ⏳ À faire |
| Phase 3 | 3 | ⏳ À faire |
| Phase 4 | 4 | ⏳ À faire |
| Phase 5 | 2 | ⏳ Optionnel |
| **Total** | **19** | - |

---

## 🎯 Commandes pour créer les issues

```bash
# Créer l'Épic
gh issue create \
  --repo Rwanbt/unifia \
  --title "[Épic] Portage Rust Pocket TTS sur Android" \
  --body "Porter le runtime Rust officiel de Pocket TTS (Kyutai) sur Android..." \
  --label enhancement,voice,rust,android,mobile,epic

# Créer une issue (exemple)
gh issue create \
  --repo Rwanbt/unifia \
  --title "[P1-1] Builder libptts_ffi.so depuis xn-ptts et tester sur PC" \
  --body "Cloner gabrieldemarmiesse/xn-ptts, builder libptts_ffi.so pour x86_64..." \
  --label phase-1,backend,rust,spike,testing \
  --assignee erwanbarat
```

---

## 📝 Notes

- **Priorité** : Les issues sont marquées `high` pour les phases 0-4, `low` pour la phase 5 (optionnelle).
- **Dépendances** : Les issues doivent être créées dans l'ordre des dépendances.
- **Estimation** : Les estimations sont indicatives et peuvent être ajustées.
- **Assigné** : Toutes les issues sont assignées à @erwanbarat par défaut.
