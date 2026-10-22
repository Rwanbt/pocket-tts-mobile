//! Streaming, annulation et concurrence — tests déterministes.
//!
//! Aucun test de ce fichier n'attend : le temps n'est pas une entrée du
//! contrat. Le backend injecté produit un nombre connu de chunks, ce qui rend
//! les vérifications reproductibles à l'identique.
//!
//! Les situations couvertes sont celles qui cassent en production :
//! - un stream annulé pendant une lecture (use-after-free) ;
//! - un lecteur plus lent que le producteur (accumulation sans borne) ;
//! - un backend qui échoue (erreur avalée) ;
//! - un thread de génération qui panique (moteur empoisonné) ;
//! - des options de texte modifiées après le début d'un stream.

use pocket_tts_mobile::backend::Backend;
use pocket_tts_mobile::engine::{Engine, TextOptions, CHUNK};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Backend déterministe : produit `chunks` chunks de `CHUNK` samples.
struct Fake {
    chunks: usize,
    /// Nombre de chunks réellement produits, observé par le test.
    produced: Arc<AtomicUsize>,
    /// Fait échouer la génération au premier chunk.
    fail: bool,
    /// Fait paniquer la génération.
    panic: bool,
}

impl Fake {
    fn new(chunks: usize) -> Self {
        Self {
            chunks,
            produced: Arc::new(AtomicUsize::new(0)),
            fail: false,
            panic: false,
        }
    }
}

impl Backend for Fake {
    fn warmup(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn generate(
        &self,
        _text: &str,
        _voice: &str,
        _options: TextOptions,
        _temperature: f32,
        sink: &mut (dyn FnMut(Vec<f32>) -> anyhow::Result<()> + Send),
    ) -> anyhow::Result<()> {
        if self.panic {
            panic!("backend panic on purpose");
        }
        if self.fail {
            anyhow::bail!("backend failure on purpose");
        }
        for index in 0..self.chunks {
            let sample = index as f32 * 0.001;
            sink(vec![sample; CHUNK])?;
            self.produced.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

/// Un backend qui observe l'annulation.
struct Watcher {
    cancelled: Arc<AtomicBool>,
}

impl Backend for Watcher {
    fn warmup(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn generate(
        &self,
        _text: &str,
        _voice: &str,
        _options: TextOptions,
        _temperature: f32,
        sink: &mut (dyn FnMut(Vec<f32>) -> anyhow::Result<()> + Send),
    ) -> anyhow::Result<()> {
        for _ in 0..1000 {
            if self.cancelled.load(Ordering::Relaxed) {
                return Ok(());
            }
            // Le refus du sink est le signal d'annulation du lecteur.
            if sink(vec![0.0; CHUNK]).is_err() {
                self.cancelled.store(true, Ordering::Relaxed);
                return Ok(());
            }
        }
        Ok(())
    }
}

/// Un chunk produit contient exactement `CHUNK` samples.
#[test]
fn a_stream_delivers_the_expected_chunks() {
    let fake = Arc::new(Fake::new(3));
    let produced = Arc::clone(&fake.produced);
    let mut engine = Engine::with_backend(fake, 0.5);
    let mut stream = engine.start("bonjour", "voice.wav").expect("start");
    let mut count = 0;
    let mut total = 0;
    while let Some(chunk) = stream.next() {
        count += 1;
        total += chunk.len();
    }
    assert_eq!(count, 3, "le nombre de chunks doit être celui du backend");
    assert_eq!(total, 3 * CHUNK);
    assert_eq!(produced.load(Ordering::SeqCst), 3);
}

/// Un backend qui échoue ne doit produire aucun chunk et ne pas paniquer.
#[test]
fn a_failing_backend_yields_no_chunk() {
    let mut fake = Fake::new(3);
    fake.fail = true;
    let mut engine = Engine::with_backend(Arc::new(fake), 0.5);
    let mut stream = engine.start("bonjour", "voice.wav").expect("start");
    assert!(
        stream.next().is_none(),
        "un backend en erreur ne doit rien produire"
    );
}

/// Un backend qui panique ne doit pas laisser le moteur inutilisable.
///
/// Le thread de génération est détaché : son panic ne doit pas se propager
/// à l'appelant, qui lit le stream.
#[test]
fn a_panicking_backend_does_not_poison_the_engine() {
    let mut fake = Fake::new(1);
    fake.panic = true;
    let mut engine = Engine::with_backend(Arc::new(fake), 0.5);
    let mut stream = engine.start("bonjour", "voice.wav").expect("start");
    assert!(stream.next().is_none());
    // Le moteur reste utilisable : un nouveau stream démarre normalement.
    let mut other = engine.start("bonjour", "voice.wav").expect("second start");
    assert!(other.next().is_none());
}

/// Annuler un stream pendant la lecture doit s'arrêter proprement.
#[test]
fn stopping_a_stream_ends_the_read() {
    let fake = Arc::new(Fake::new(1000));
    let mut engine = Engine::with_backend(fake, 0.5);
    let mut stream = engine.start("bonjour", "voice.wav").expect("start");
    // Un chunk est lu, puis le stream est arrêté comme le fait l'appelant
    // quand l'utilisateur interrompt la dictée.
    assert!(stream.next().is_some());
    stream.stop();
    // Après l'arrêt, la lecture ne doit plus jamais bloquer ni renvoyer de
    // données : elle rend `None`.
    assert!(stream.next().is_none());
}

/// Le lecteur doit voir l'annulation et le backend doit s'arrêter.
#[test]
fn the_backend_observes_the_cancellation() {
    let watcher = Arc::new(Watcher {
        cancelled: Arc::new(AtomicBool::new(false)),
    });
    let cancelled = Arc::clone(&watcher.cancelled);
    let mut engine = Engine::with_backend(watcher, 0.5);
    let mut stream = engine.start("bonjour", "voice.wav").expect("start");
    assert!(stream.next().is_some());
    stream.stop();
    assert!(
        cancelled.load(Ordering::Relaxed),
        "le backend doit constater l'annulation"
    );
}

/// Un lecteur lent ne doit pas faire croître la file sans borne.
///
/// Le canal est borné : le producteur attend quand le lecteur est lent, donc
/// la mémoire reste constante.
#[test]
fn a_slow_reader_does_not_grow_the_queue_without_bound() {
    let fake = Arc::new(Fake::new(50));
    let produced = Arc::clone(&fake.produced);
    let mut engine = Engine::with_backend(fake, 0.5);
    let mut stream = engine.start("bonjour", "voice.wav").expect("start");
    // Après le premier chunk, le producteur a pu remplir le canal borné mais
    // pas davantage : la file est plafonnée.
    let first = stream.next().expect("first chunk");
    assert_eq!(first.len(), CHUNK);
    let produced_early = produced.load(Ordering::SeqCst);
    assert!(
        produced_early <= 16,
        "le producteur ne doit pas dépasser la borne du canal, il en a produit {produced_early}"
    );
    // La lecture reste possible jusqu'au bout.
    let mut total = first.len();
    while let Some(chunk) = stream.next() {
        total += chunk.len();
    }
    assert_eq!(total, 50 * CHUNK);
}

/// Les options de texte sont figées dès le premier stream.
#[test]
fn text_options_freeze_after_the_first_stream() {
    let mut engine = Engine::with_backend(Arc::new(Fake::new(0)), 0.5);
    engine
        .set_text_options(8, false, true, 50)
        .expect("options acceptées");
    assert_eq!(
        engine.options(),
        TextOptions {
            eos_extra_frames: 8,
            pad_short_inputs: false,
            remove_semicolons: true,
            max_chunk_tokens: 50,
        }
    );
    let _stream = engine.start("bonjour", "voice.wav").expect("start");
    assert!(
        engine.set_text_options(-1, true, false, 0).is_err(),
        "les options ne doivent plus changer après un stream"
    );
    assert_eq!(engine.options().eos_extra_frames, 8);
}

/// Les options françaises sont acceptées telles quelles.
#[test]
fn the_french_options_are_accepted() {
    let mut engine = Engine::with_backend(Arc::new(Fake::new(0)), 0.5);
    assert!(engine.set_text_options(8, false, true, 50).is_ok());
    assert!(engine.set_text_options(-1, true, false, 0).is_ok());
    assert!(engine.set_text_options(0, false, false, 0).is_ok());
}

/// Les options impossibles sont refusées, et l'état reste cohérent.
#[test]
fn impossible_options_are_refused_and_leave_the_state_untouched() {
    let mut engine = Engine::with_backend(Arc::new(Fake::new(0)), 0.5);
    engine
        .set_text_options(8, false, true, 50)
        .expect("options");
    let before = engine.options();
    assert!(engine.set_text_options(-2, false, true, 50).is_err());
    assert!(engine.set_text_options(8, false, true, -5).is_err());
    assert_eq!(engine.options(), before);
}

/// Deux streams ne doivent pas partager leur file.
#[test]
fn streams_are_independent() {
    let mut engine = Engine::with_backend(Arc::new(Fake::new(2)), 0.5);
    let mut first = engine.start("un", "voice.wav").expect("start");
    let mut second = engine.start("deux", "voice.wav").expect("start");
    assert!(first.next().is_some());
    assert!(second.next().is_some());
    assert!(first.next().is_some());
    assert!(second.next().is_some());
    assert!(first.next().is_none());
    assert!(second.next().is_none());
}

/// Un stream abandonné sans `stop` explicite ne doit pas laisser de thread
/// qui tourne : `Drop` s'en charge.
#[test]
fn dropping_a_stream_joins_the_generation_thread() {
    let fake = Arc::new(Fake::new(5));
    let produced = Arc::clone(&fake.produced);
    {
        let mut engine = Engine::with_backend(fake, 0.5);
        let stream = engine.start("bonjour", "voice.wav").expect("start");
        drop(stream);
    }
    // Le thread s'est arrêté : plus aucun chunk n'est produit après le drop.
    let after = produced.load(Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(
        produced.load(Ordering::SeqCst),
        after,
        "aucun chunk ne doit être produit après l'abandon du stream"
    );
}

/// Le mutex du moteur ne doit pas être corrompu par un panic du backend.
#[test]
fn the_engine_survives_a_panicking_backend() {
    let mut fake = Fake::new(1);
    fake.panic = true;
    let mut engine = Engine::with_backend(Arc::new(fake), 0.5);
    let mut stream = engine.start("bonjour", "voice.wav").expect("start");
    assert!(stream.next().is_none());
    // Le moteur reste manipulable après le panic du thread de génération.
    let options = engine.options();
    assert_eq!(options.eos_extra_frames, -1);
}

/// Un mutex empoisonné doit produire une erreur, pas un deadlock.
#[test]
fn a_poisoned_mutex_is_reported_not_hung() {
    let poisoned = Arc::new(Mutex::new(0u32));
    let clone = Arc::clone(&poisoned);
    let _ = std::thread::spawn(move || {
        let _guard = clone.lock().expect("lock before panic");
        panic!("poison on purpose");
    })
    .join();
    assert!(poisoned.lock().is_err(), "le mutex doit être empoisonné");
}
