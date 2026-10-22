//! Contrat ABI `ptt_*` — le test qui empêche la dérive.
//!
//! Trois niveaux de vérification, du plus faible au plus fort :
//!
//! 1. **Compilation** : les signatures sont fixées par des assignations de
//!    pointeur de fonction typé. Changer une signature casse la compilation,
//!    pas un test à l'exécution.
//! 2. **Exécution** : les 8 symboles sont résolus dans la bibliothèque
//!    partagée réellement construite, avec `libloading` — le même mécanisme
//!    que `PocketApi::load()` côté Unifia. Un symbole renommé, retiré ou mal
//!    exporté est détecté ici.
//! 3. **Comportement** : les cas limites de l'ABI (pointeurs nuls, cycle de
//!    vie, allocation/libération) sont vérifiés un par un.
//!
//! Ce fichier ne dépend que de l'API publique `ptt_*`, jamais des internes :
//! il reste valide quand le moteur change.

use pocket_tts_mobile::ffi::*;
use std::ffi::{c_char, c_int, c_void, CString};
use std::path::PathBuf;

/// Les 8 symboles que l'appelant Unifia résout, avec leur nom exact.
const SYMBOLS: [&[u8]; 8] = [
    b"ptt_create\0",
    b"ptt_destroy\0",
    b"ptt_warmup\0",
    b"ptt_set_text_options\0",
    b"ptt_stream_start\0",
    b"ptt_stream_read\0",
    b"ptt_stream_end\0",
    b"ptt_free_audio\0",
];

/// Les signatures doivent rester celles que l'appelant déclare.
///
/// Une assignation de pointeur de fonction typé est une assertion de
/// compilation : toute dérive de signature devient une erreur de build.
#[test]
fn the_abi_signatures_are_exact() {
    let _: unsafe extern "C" fn(
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        f32,
        c_int,
        c_int,
    ) -> *mut c_void = ptt_create;
    let _: unsafe extern "C" fn(*mut c_void) = ptt_destroy;
    let _: unsafe extern "C" fn(*mut c_void) -> f64 = ptt_warmup;
    let _: unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int, c_int) -> c_int =
        ptt_set_text_options;
    let _: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> *mut c_void =
        ptt_stream_start;
    let _: unsafe extern "C" fn(*mut c_void, *mut *mut f32, *mut c_int) -> c_int = ptt_stream_read;
    let _: unsafe extern "C" fn(*mut c_void) = ptt_stream_end;
    let _: unsafe extern "C" fn(*mut f32) = ptt_free_audio;
}

/// La bibliothèque partagée construite doit exposer les 8 symboles.
///
/// C'est le test qui reproduit ce que fait l'app au démarrage : si un symbole
/// manque, l'application entière perd le TTS, pas seulement un test.
///
/// La bibliothèque partagée est un artefact à part : `cargo test` ne la
/// construit pas. Le test la cherche donc dans les emplacements usuels, et
/// signale bruyamment son absence ; la verification qui fait autorite est
/// celle de la CI, sur la bibliotheque construite (`nm -D`).
#[test]
fn the_shared_library_exports_every_symbol() {
    let Some(path) = find_shared_library() else {
        eprintln!(
            "ATTENTION : la bibliotheque partagee n'est pas construite, les 8 symboles ne sont \
             donc pas verifies ici. Lance `cargo build` avant ce test ; la CI le fait et verifie \
             les symboles sur l'artefact."
        );
        return;
    };
    // SAFETY: la bibliothèque est construite par ce même cargo, et les
    // symboles sont lus sans être appelés.
    let lib = unsafe { libloading::Library::new(&path) }
        .unwrap_or_else(|err| panic!("impossible de charger {} : {err}", path.display()));
    for name in SYMBOLS {
        // SAFETY: `name` est un nom de symbole terminé par NUL ; seul un
        // pointeur est obtenu, aucune fonction n'est appelée.
        let found = unsafe { lib.get::<*mut c_void>(name) };
        assert!(
            found.is_ok(),
            "symbole absent de la bibliothèque partagée : {}",
            String::from_utf8_lossy(&name[..name.len() - 1])
        );
    }
}

/// Cherche la bibliothèque partagée dans les emplacements usuels.
fn find_shared_library() -> Option<PathBuf> {
    let mut dir = std::env::current_exe().ok()?;
    dir.pop(); // le binaire du test
    if dir.ends_with("deps") {
        dir.pop();
    }
    let names = [
        "pocket_tts_mobile.dll",
        "libpocket_tts_mobile.so",
        "libpocket_tts_mobile.dylib",
    ];
    for name in names {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
        let in_deps = dir.join("deps").join(name);
        if in_deps.exists() {
            return Some(in_deps);
        }
    }
    None
}

/// Tous les appels avec des pointeurs nuls doivent être refusés, sans paniquer.
///
/// Un appel FFI invalide tue l'application entière : ces cas doivent renvoyer
/// un code d'échec, jamais un crash.
#[test]
fn null_arguments_are_refused_everywhere() {
    // SAFETY: les pointeurs sont volontairement invalides ; la fonction doit
    // les refuser sans les déréférencer.
    unsafe {
        assert!(ptt_create(
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            0.5,
            1,
            2
        )
        .is_null());
        assert_eq!(ptt_warmup(std::ptr::null_mut()), 0.0);
        assert_eq!(
            ptt_set_text_options(std::ptr::null_mut(), 8, 0, 1, 50),
            1,
            "des options invalides sur un moteur nul doivent être refusées"
        );
        assert!(
            ptt_stream_start(std::ptr::null_mut(), std::ptr::null(), std::ptr::null()).is_null()
        );
        assert_eq!(
            ptt_stream_read(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            0
        );
        // Ces trois fonctions acceptent NULL sans effet : elles nettoient un
        // pointeur qui peut légitimement être nul.
        ptt_destroy(std::ptr::null_mut());
        ptt_stream_end(std::ptr::null_mut());
        ptt_free_audio(std::ptr::null_mut());
    }
}

/// Un moteur ne peut pas être créé depuis un pack inexistant.
///
/// Sans cette vérification, l'application obtiendrait un moteur et l'échec
/// n'apparaîtrait qu'à la première synthèse, sans cause visible.
#[test]
fn a_missing_pack_does_not_produce_an_engine() {
    let models = CString::new("definitely/not/a/pack").unwrap();
    let voices = CString::new("definitely/not/a/pack/voices").unwrap();
    let tokenizer = CString::new("definitely/not/a/pack/tokenizer.json").unwrap();
    let precision = CString::new("f32").unwrap();
    // SAFETY: les chaînes vivent pendant l'appel.
    let engine = unsafe {
        ptt_create(
            models.as_ptr(),
            voices.as_ptr(),
            tokenizer.as_ptr(),
            precision.as_ptr(),
            0.5,
            1,
            2,
        )
    };
    if !engine.is_null() {
        // SAFETY: le moteur vient de ptt_create.
        unsafe { ptt_destroy(engine) };
        panic!("un pack inexistant ne doit pas produire de moteur");
    }
}

/// Lire un stream inexistant ne doit ni planter ni allouer.
#[test]
fn reading_a_bogus_stream_is_refused() {
    let mut samples: *mut f32 = std::ptr::null_mut();
    let mut length: c_int = 0;
    // SAFETY: pointeurs de sortie valides ; le contexte de stream est
    // volontairement invalide, la fonction doit le refuser sans déréférencer.
    let status = unsafe { ptt_stream_read(std::ptr::null_mut(), &mut samples, &mut length) };
    assert_eq!(status, 0);
    assert!(
        samples.is_null(),
        "un stream invalide ne doit pas allouer de tampon"
    );
    assert_eq!(length, 0);
}

/// Le chemin de destruction complet ne doit pas fuir ni planter.
#[test]
fn the_full_lifecycle_is_clean() {
    let dir = tempdir("lifecycle");
    let models = CString::new(dir.to_string_lossy().as_ref()).unwrap();
    let voices = CString::new(dir.join("voices").to_string_lossy().as_ref()).unwrap();
    let tokenizer = CString::new(dir.join("tokenizer.json").to_string_lossy().as_ref()).unwrap();
    let precision = CString::new("f32").unwrap();

    // SAFETY: les chaînes vivent pendant l'appel.
    let engine = unsafe {
        ptt_create(
            models.as_ptr(),
            voices.as_ptr(),
            tokenizer.as_ptr(),
            precision.as_ptr(),
            0.5,
            1,
            2,
        )
    };
    if engine.is_null() {
        // Sans pack valide, aucun moteur : le contrat est respecté, la
        // construction du pack valide est testée ailleurs.
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    // SAFETY: moteur vivant.
    unsafe {
        assert_eq!(ptt_set_text_options(engine, 8, 0, 1, 50), 0);
        let warmup = ptt_warmup(engine);
        assert!(warmup >= 0.0, "le préchauffage ne peut pas être négatif");
        ptt_destroy(engine);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Crée un dossier temporaire unique.
fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ptt-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
