//! Pont entre les options de l'ABI et le moteur `ptts`.
//!
//! # Le problème que ce module résout
//!
//! Le moteur `ptts` applique son propre front-end texte
//! (`ptts::tts_model::prepare_text_prompt`), qui rend **3 frames** après l'EOS
//! pour un texte court et **1 frame** au-delà. Le runtime officiel de Kyutai,
//! lui, applique la valeur recommandée par la config du modèle :
//! `model_recommended_frames_after_eos: 8` pour le français 24 couches.
//!
//! C'est la cause du défaut « mot final avalé » : avec 1 ou 3 frames au lieu
//! de 8, la syllabe finale n'a pas le temps d'être rendue. Sur 18 rendus int8
//! transcrits, l'ouverture « Bonjour le monde. » disparaissait dans 6 cas sur
//! 6 et le WER moyen atteignait 0.101 ; avec 8 frames, elle était conservée
//! dans 6 cas sur 6 et le WER tombait à 0.048.
//!
//! L'API de haut niveau de `ptts` (`Synth::stream_with`) ne permet pas de
//! choisir ce nombre : il est calculé à l'intérieur. La cdylib doit donc
//! piloter la génération de plus bas niveau (`TTSModel` + `plan::EosPolicy`),
//! comme le fait l'adaptateur Kyutai officiel.
//!
//! Ce module porte la règle de calcul, isolée et testable.

use crate::engine::TextOptions;

/// Calcule le nombre de frames rendues après l'EOS.
///
/// Trois sources, par ordre de priorité, comme dans l'adaptateur officiel :
///
/// 1. la valeur demandée par l'appelant (`ptt_set_text_options`) ;
/// 2. la valeur recommandée par la config du modèle (`8` pour le français) ;
/// 3. l'estimation du front-end (1 ou 3) augmentée de deux frames.
///
/// # Arguments
/// - `requested` : la valeur de l'appelant ; `-1` signifie « laisse le runtime
///   décider ».
/// - `recommended` : `model_recommended_frames_after_eos` de la config.
/// - `guess` : l'estimation du front-end (1 ou 3).
pub fn frames_after_eos(requested: i32, recommended: Option<usize>, guess: usize) -> usize {
    if requested >= 0 {
        return requested as usize;
    }
    if let Some(value) = recommended {
        return value;
    }
    guess + 2
}

/// Nombre de tokens par chunk de phrase.
///
/// `0` signifie « une phrase par génération », comme dans l'ABI d'origine.
pub fn max_chunk_tokens(requested: i32, recommended: usize) -> usize {
    if requested > 0 {
        return requested as usize;
    }
    recommended
}

/// Les options de texte effectives, config d'abord, ABI ensuite.
///
/// L'ABI surcharge la config : c'est le moyen pour l'appelant de corriger un
/// modèle sans modifier le pack.
pub fn effective_text_options(
    base: &crate::kyutai::text::TextOptions,
    requested: &TextOptions,
) -> crate::kyutai::text::TextOptions {
    let mut out = base.clone();
    out.pad_with_spaces_for_short_inputs = requested.pad_short_inputs;
    out.remove_semicolons = requested.remove_semicolons;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_requested_value_wins_over_everything() {
        assert_eq!(frames_after_eos(8, Some(8), 3), 8);
        assert_eq!(frames_after_eos(0, Some(8), 3), 0);
        assert_eq!(frames_after_eos(3, None, 1), 3);
    }

    #[test]
    fn the_config_recommendation_wins_over_the_guess() {
        assert_eq!(frames_after_eos(-1, Some(8), 3), 8);
        assert_eq!(frames_after_eos(-1, Some(8), 1), 8);
    }

    #[test]
    fn the_guess_plus_two_is_the_last_resort() {
        assert_eq!(frames_after_eos(-1, None, 3), 5);
        assert_eq!(frames_after_eos(-1, None, 1), 3);
    }

    #[test]
    fn the_chunk_size_falls_back_to_the_recommendation() {
        assert_eq!(max_chunk_tokens(50, 50), 50);
        assert_eq!(max_chunk_tokens(0, 50), 50);
        assert_eq!(max_chunk_tokens(-1, 50), 50);
        assert_eq!(max_chunk_tokens(20, 50), 20);
    }

    #[test]
    fn the_abi_overrides_only_the_two_text_options() {
        let base = crate::kyutai::text::TextOptions {
            pad_with_spaces_for_short_inputs: true,
            remove_semicolons: false,
            append_terminal_punctuation: true,
            capitalize_first_letter: true,
            replace_characters: Default::default(),
        };
        let requested = TextOptions {
            eos_extra_frames: 8,
            pad_short_inputs: false,
            remove_semicolons: true,
            max_chunk_tokens: 50,
        };
        let out = effective_text_options(&base, &requested);
        assert!(!out.pad_with_spaces_for_short_inputs);
        assert!(out.remove_semicolons);
        // Les options que l'ABI ne porte pas restent celles de la config.
        assert!(out.append_terminal_punctuation);
        assert!(out.capitalize_first_letter);
    }
}
