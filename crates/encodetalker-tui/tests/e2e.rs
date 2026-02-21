/// Tests E2E du TUI via pseudo-terminal (expectrl)
///
/// Ces tests lancent le vrai binaire TUI dans un PTY, envoient des touches
/// clavier automatiquement, et vérifient les fichiers produits sur disque.
///
/// Pré-requis : les binaires doivent être compilés au préalable.
///   cargo build -p encodetalker-tui -p encodetalker-daemon
///
/// Lancement :
///   cargo test -p encodetalker-tui -- --ignored
use expectrl::{Expect, Session};
use std::path::PathBuf;
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

/// Racine du workspace (remonte depuis crates/encodetalker-tui/)
fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Chemin vers le binaire TUI compilé par Cargo
fn tui_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_encodetalker-tui"))
}

/// Chemin vers le binaire daemon (même dossier que le TUI, comme en production)
fn daemon_bin() -> PathBuf {
    tui_bin()
        .parent()
        .expect("Le binaire TUI n'a pas de dossier parent")
        .join("encodetalker-daemon")
}

/// Touches spéciales en séquences d'échappement ANSI (mode raw terminal)
const KEY_DOWN: &[u8] = b"\x1b[B";
const KEY_RIGHT: &[u8] = b"\x1b[C";
const KEY_ENTER: &[u8] = b"\r";

/// Test E2E : encoder test1.mkv avec SVT-AV1 preset 13 via le TUI
///
/// Séquence de touches :
///   ↓           → sélectionner test1.mkv (2ème entrée après "..")
///   Enter       → ouvrir le dialogue de configuration
///   ↓↓↓         → naviguer jusqu'au champ Preset (field 3)
///   →×7         → augmenter le preset de 6 (défaut) à 13
///   Enter       → soumettre le job
///
/// Le test attend ensuite jusqu'à 5 minutes l'apparition du fichier
/// test1.av1.mkv, puis vérifie qu'il existe et n'est pas vide.
#[test]
#[ignore] // Test lent (encodage réel) — lancer avec: cargo test -- --ignored
fn test_tui_encode_test1_svtav1_preset13() {
    let tui = tui_bin();
    let daemon = daemon_bin();

    // --- Vérifications préalables ---
    assert!(
        tui.exists(),
        "Binaire TUI introuvable : {}\nLancer 'cargo build -p encodetalker-tui' d'abord",
        tui.display()
    );
    assert!(
        daemon.exists(),
        "Binaire daemon introuvable : {}\nLancer 'cargo build -p encodetalker-daemon' d'abord",
        daemon.display()
    );

    let input = project_root().join("vidéos_de_test").join("test1.mkv");
    assert!(
        input.exists(),
        "Vidéo de test introuvable : {}",
        input.display()
    );

    // --- Dossier temporaire avec uniquement test1.mkv ---
    // Cela garantit une navigation déterministe dans le file browser :
    // seules 2 entrées : ".." (index 0) et "test1.mkv" (index 1).
    let tmp_dir = tempfile::tempdir().expect("Impossible de créer le dossier temporaire");
    let link = tmp_dir.path().join("test1.mkv");
    std::os::unix::fs::symlink(&input, &link)
        .expect("Impossible de créer le lien symbolique vers test1.mkv");

    let output = tmp_dir.path().join("test1.av1.mkv");

    // Nettoyer un éventuel fichier output résiduel
    if output.exists() {
        std::fs::remove_file(&output).expect("Impossible de supprimer l'output résiduel");
    }

    // --- Lancer le TUI dans un pseudo-terminal ---
    let mut cmd = Command::new(&tui);
    cmd.current_dir(tmp_dir.path());

    let mut session = Session::spawn(cmd).expect("Impossible de lancer le TUI dans un PTY");

    // Attendre que le TUI démarre : connexion au daemon, chargement des dépendances
    // Le daemon est démarré automatiquement par le TUI si nécessaire.
    sleep(Duration::from_secs(7));

    // --- Séquence de touches ---

    // ↓ : sélectionner test1.mkv (entrée 1, après ".." à l'entrée 0)
    session.send(KEY_DOWN).expect("Envoi ↓ (sélection fichier)");
    sleep(Duration::from_millis(300));

    // Enter : ouvrir le dialogue de configuration d'encodage
    session
        .send(KEY_ENTER)
        .expect("Envoi Enter (ouvrir config)");
    sleep(Duration::from_millis(500));

    // ↓×3 : naviguer du champ 0 (Encoder) au champ 3 (Preset)
    //   field 0 → Encoder   (SVT-AV1 par défaut, on garde)
    //   field 1 → Audio     (Opus 128kbps par défaut, on garde)
    //   field 2 → CRF       (30 par défaut, on garde)
    //   field 3 → Preset    (6 par défaut, on va modifier)
    for _ in 0..3 {
        session.send(KEY_DOWN).expect("Envoi ↓ (navigation champ)");
        sleep(Duration::from_millis(150));
    }

    // →×7 : augmenter le preset de 6 (défaut) à 13
    // Preset 13 = encodage le plus rapide pour SVT-AV1
    for _ in 0..7 {
        session
            .send(KEY_RIGHT)
            .expect("Envoi → (incrémenter preset)");
        sleep(Duration::from_millis(100));
    }

    // Enter : valider la configuration et lancer l'encodage
    session
        .send(KEY_ENTER)
        .expect("Envoi Enter (soumettre job)");
    sleep(Duration::from_millis(500));

    // --- Attendre la fin de l'encodage (timeout 5 minutes) ---
    println!(
        "Job soumis. Attente du fichier output : {}",
        output.display()
    );

    let timeout = Duration::from_secs(5 * 60);
    let deadline = Instant::now() + timeout;

    loop {
        if output.exists() {
            println!(
                "Fichier output détecté après {:?}",
                deadline - Instant::now()
            );
            break;
        }

        assert!(
            Instant::now() < deadline,
            "Timeout : test1.av1.mkv n'a pas été créé dans les 5 minutes.\n\
             Dossier temp : {}",
            tmp_dir.path().display()
        );

        sleep(Duration::from_secs(5));
    }

    // --- Vérifications ---
    assert!(
        output.exists(),
        "Le fichier output n'existe pas : {}",
        output.display()
    );

    let size = output
        .metadata()
        .expect("Impossible de lire les métadonnées du fichier output")
        .len();
    assert!(
        size > 0,
        "Le fichier output est vide : {}",
        output.display()
    );

    println!(
        "✅ Encodage réussi : {} ({} octets)",
        output.display(),
        size
    );

    // --- Quitter le TUI proprement ---
    session.send(b"q").ok();
    sleep(Duration::from_millis(500));
    session.send(b"y").ok();
    sleep(Duration::from_millis(300));
}
