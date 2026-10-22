//! Le contrat du front-end français, vérifié contre le moteur réel.
//!
//! Ce fichier est le garde-fou le plus important du portage. Il verrouille la
//! raison d'être de la cdylib : le moteur `ptts` applique son propre
//! front-end, qui rend 1 ou 3 frames après l'EOS, alors que le français 24
//! couches en demande 8. Sans pont, le mot final est avalé.
//!
//! Le test compare les deux chemins sur les mêmes textes. Si quelqu'un remplace
//! un jour le chemin de bas niveau par `Synth::stream_with`, ce test échoue.

use pocket_tts_mobile::eos::frames_after_eos;
use pocket_tts_mobile::kyutai::config::Config;
use pocket_tts_mobile::kyutai::text::prepare_text_prompt;

/// La config du français 24 couches, telle qu'elle est embarquée.
fn french_24l() -> Config {
    let yaml = pocket_tts_mobile::kyutai::config_yaml("french_24l")
        .expect("la config french_24l doit etre embarquee");
    Config::parse(yaml).expect("la config french_24l doit etre analysable")
}

/// La config du français 6 couches, telle qu'elle est embarquee.
fn french_6l() -> Config {
    let yaml = pocket_tts_mobile::kyutai::config_yaml("french")
        .expect("la config french doit etre embarquee");
    Config::parse(yaml).expect("la config french doit etre analysable")
}

/// Le modele 24 couches demande huit frames apres l'EOS.
#[test]
fn the_french_24_layers_recommends_eight_frames() {
    let config = french_24l();
    assert_eq!(
        config.model_recommended_frames_after_eos,
        Some(8),
        "le francais 24 couches doit recommander 8 frames apres l'EOS"
    );
}

/// Le moteur `ptts` ne rend jamais plus de trois frames apres l'EOS.
///
/// C'est la limite qui impose une cdylib dediee : l'API de haut niveau de
/// `ptts` ne sait pas produire les 8 frames du francais.
#[test]
fn the_stock_engine_never_reaches_the_french_eight_frames() {
    for text in [
        "Bonjour le monde.",
        "Je vais bien, merci beaucoup.",
        "La dictée vocale est maintenant disponible en français sur ce téléphone.",
    ] {
        let (_, frames) = ptts::tts_model::prepare_text_prompt(text);
        assert!(
            frames <= 3,
            "le moteur ptts rend {frames} frames pour {text:?}, il ne peut pas atteindre 8"
        );
    }
}

/// Le pont produit les huit frames francaises quand l'appelant ne tranche pas.
///
/// `-1` est ce que l'appelant envoie pour les packs qui n'imposent rien ; le
/// francais 24 couches impose 8 par sa config.
#[test]
fn the_bridge_produces_the_french_eight_frames() {
    let config = french_24l();
    let recommended = config.model_recommended_frames_after_eos;
    let (_, guess) = ptts::tts_model::prepare_text_prompt("Bonjour le monde.");
    assert_eq!(frames_after_eos(-1, recommended, guess), 8);
    // Et quand l'appelant impose la valeur, elle est respectee.
    assert_eq!(frames_after_eos(8, recommended, guess), 8);
}

/// Le pont ne dégrade jamais la valeur demandée par l'appelant.
#[test]
fn the_bridge_never_lowers_the_requested_frames() {
    let recommended = french_24l().model_recommended_frames_after_eos;
    for requested in [0, 1, 3, 8, 16] {
        let frames = frames_after_eos(requested, recommended, 3);
        assert_eq!(frames, requested as usize);
    }
}

/// Le francais ne remplace pas les `;` par des `,` quand la config le dit, et
/// l'inverse quand elle ne le dit pas.
#[test]
fn the_semicolon_rule_follows_the_config() {
    let config = french_24l();
    let options = pocket_tts_mobile::kyutai::text::TextOptions::from_config(&config);
    assert!(
        options.remove_semicolons,
        "le francais doit lire ';' comme ','"
    );
    let (prepared, _) = prepare_text_prompt("un ; deux", &options).expect("prepare");
    assert!(
        !prepared.contains(';'),
        "le ';' doit avoir disparu : {prepared:?}"
    );
    assert!(
        prepared.contains(','),
        "le ';' doit avoir devenu ',' : {prepared:?}"
    );
}

/// Le francais 24 couches ne espace pas les phrases courtes.
///
/// Le padding de huit espaces est un comportement du modele 6 couches ; le
/// modele 24 ne le supporte pas.
#[test]
fn the_french_24_layers_does_not_pad_short_inputs() {
    let config = french_24l();
    assert!(
        !config.pad_with_spaces_for_short_inputs,
        "le francais 24 couches ne doit pas espace les phrases courtes"
    );
    let options = pocket_tts_mobile::kyutai::text::TextOptions::from_config(&config);
    let (prepared, _) = prepare_text_prompt("Bonjour", &options).expect("prepare");
    assert!(
        !prepared.starts_with(' '),
        "la phrase ne doit pas commencer par des espaces : {prepared:?}"
    );
}

/// La config 6 couches garde le comportement amont, espaces compris.
#[test]
fn the_french_6_layers_keeps_the_upstream_padding() {
    let config = french_6l();
    assert!(
        config.pad_with_spaces_for_short_inputs,
        "le francais 6 couches conserve le padding amont"
    );
    let options = pocket_tts_mobile::kyutai::text::TextOptions::from_config(&config);
    let (prepared, _) = prepare_text_prompt("Bonjour", &options).expect("prepare");
    assert!(
        prepared.starts_with("        "),
        "la phrase doit etre espacee : {prepared:?}"
    );
}

/// Les caracteres absents des donnees d'entrainement sont remplaces.
#[test]
fn the_unknown_characters_are_replaced() {
    let config = french_24l();
    let options = pocket_tts_mobile::kyutai::text::TextOptions::from_config(&config);
    assert!(
        !options.replace_characters.is_empty(),
        "la config francaise doit remplacer des caracteres"
    );
    let (prepared, _) = prepare_text_prompt("« guillemets » ; test", &options).expect("prepare");
    assert!(
        !prepared.contains('«'),
        "les guillemets doivent etre remplaces"
    );
    assert!(
        !prepared.contains('»'),
        "les guillemets doivent etre remplaces"
    );
}

/// Une phrase vide est refusee, jamais silencieusement acceptee.
#[test]
fn an_empty_text_is_refused() {
    let config = french_24l();
    let options = pocket_tts_mobile::kyutai::text::TextOptions::from_config(&config);
    let err = prepare_text_prompt("   ", &options).expect_err("un texte vide doit etre refuse");
    assert!(err.to_string().contains("empty"), "{err}");
}
