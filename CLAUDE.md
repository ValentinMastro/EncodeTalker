# CLAUDE.md - Guide de développement EncodeTalker

Ce fichier contient les instructions et conventions pour travailler sur le projet EncodeTalker.

## 📋 Vue d'ensemble

**EncodeTalker** est un wrapper Rust autour de ffmpeg avec interface TUI pour gérer l'encodage vidéo en AV1. Le projet utilise une architecture client-serveur où :
- Le **daemon** gère la queue d'encodage en arrière-plan
- Le **TUI** est une interface reconnectable pour interagir avec le daemon

## 🏗️ Architecture

### Structure du workspace

```
EncodeTalker/
├── crates/
│   ├── encodetalker-common/     # Types partagés, protocole IPC
│   ├── encodetalker-daemon/     # Daemon d'encodage (serveur)
│   └── encodetalker-tui/        # Interface TUI (client)
├── scripts/
│   ├── INSTALL_DEPENDENCIES.sh  # Script de compilation des dépendances (FFmpeg, libvmaf, etc.)
│   └── CHECK_INSTALLED_DEPENDENCIES.sh
├── config/
│   └── config.toml              # Configuration par défaut
├── README.md                    # Documentation utilisateur
├── CONTRIBUTING.md              # Guide de contribution
└── CLAUDE.md                    # Ce fichier

```

### Pipeline d'encodage

```
1. ffmpeg (demux + raw video) → pipe → Encodeur AV1 → video.ivf
2. ffmpeg (extract audio) → audio.opus (ou copy)
3. ffmpeg (mux video.ivf + audio + subtitles) → output.mkv
```

**Note importante** : On utilise ffmpeg pour le muxing final, PAS mkvtoolnix/mkvmerge.

## 🔧 Compilation et test

### Compilation

```bash
# Développement (avec debug)
cargo build

# Release (optimisé)
cargo build --release

# Compiler un crate spécifique
cargo build --release -p encodetalker-daemon
cargo build --release -p encodetalker-tui
```

### Tests

```bash
# Lancer tous les tests
cargo test --all

# Tests d'un crate spécifique
cargo test -p encodetalker-common
```

### Linting et formatage

```bash
# Formatage (à faire AVANT de committer)
cargo fmt --all

# Linting (corriger tous les warnings, mode pedantic)
cargo clippy --all-targets --all-features -- -W clippy::pedantic
```

## 🚀 Lancement

### Daemon

```bash
# Lancement avec logs
RUST_LOG=info ./target/release/encodetalker-daemon

# Logs debug
RUST_LOG=debug ./target/release/encodetalker-daemon

# Logs d'un module spécifique
RUST_LOG=encodetalker_daemon::encoder=debug ./target/release/encodetalker-daemon
```

### TUI

```bash
# Le TUI démarre automatiquement le daemon si nécessaire
./target/release/encodetalker-tui
```

## 📦 Dépendances

### Gestion des dépendances

**Important** : Les dépendances sont gérées par le script bash `scripts/INSTALL_DEPENDENCIES.sh`, PAS par un crate Rust. Le script télécharge, compile et installe les dépendances localement.

### Dépendances compilées/téléchargées automatiquement

**Linux** : Le script compile ces dépendances localement (par défaut dans `.dependencies/` à côté de l'exécutable) :

1. **libvmaf** (~5 min) - Calcul de qualité vidéo (VMAF, PSNR, SSIM)
2. **FFmpeg** (15-20 min) - Demux, muxing, extraction audio
3. **SVT-AV1-PSY** (10-15 min) - Encodeur AV1 optimisé (par défaut)
4. **libaom** (15-20 min) - Encodeur AV1 de référence
5. **libopus, libvpx, libdav1d** - Codecs audio/vidéo

**Temps total de compilation : ~45-60 minutes**

Installation manuelle :
```bash
./scripts/INSTALL_DEPENDENCIES.sh           # Tout installer
./scripts/INSTALL_DEPENDENCIES.sh --vmaf    # Réinstaller libvmaf seulement
./scripts/INSTALL_DEPENDENCIES.sh --ffmpeg  # Réinstaller FFmpeg seulement
```

**Windows** : Les binaires pré-compilés sont téléchargés automatiquement dans `%LOCALAPPDATA%\encodetalker\deps\` :

1. **FFmpeg** (~2-3 min download) - Depuis GitHub Releases
2. **SVT-AV1-PSY** - À venir
3. **libaom** - À venir

**Temps total : ~2-3 minutes** (téléchargement uniquement)

### Dépendances système requises

**Linux** :
```bash
# Sur Arch/Manjaro
sudo pacman -S base-devel cmake git nasm

# Sur Ubuntu/Debian
sudo apt install build-essential cmake git nasm
```

**Windows** :
```powershell
# Installer Git et CMake via Chocolatey
choco install git cmake

# Ou télécharger manuellement depuis les sites officiels
```

## 🎯 Conventions de code

### Style Rust

- Suivre les conventions Rust standards (enforced par `rustfmt`)
- Utiliser `anyhow` pour la propagation d'erreurs
- Utiliser `thiserror` pour les erreurs custom (voir `DepsError`)
- Préférer `&Path` à `&PathBuf` dans les signatures de fonctions

### Logging

```rust
use tracing::{info, warn, error, debug};

info!("Message informatif");
warn!("Avertissement");
error!("Erreur: {}", e);
debug!("Debug détaillé");
```

### Async

- Utiliser `tokio` pour toutes les opérations async
- Préférer `tokio::fs` à `std::fs` pour les I/O
- Utiliser `tokio::process::Command` pour lancer des processus

## 🔍 Points d'attention

### 1. Compilation des dépendances

**IMPORTANT** : Ne jamais ajouter de dépendances qui nécessitent sudo pour compiler.

- ✅ Bon : Télécharger et compiler localement
- ❌ Mauvais : Nécessiter ruby, rake, boost, gmp, etc. installés sur le système

**Problème libvmaf + FFmpeg** : libvmaf contient du code C++ (svm.cpp) mais FFmpeg utilise gcc. Il faut ajouter `-lstdc++` dans `--extra-libs` lors du configure de FFmpeg, sinon le test de détection de libvmaf échoue avec des erreurs de symboles C++ non résolus (`undefined reference to std::...`).

```bash
# ✅ Configuration correcte de FFmpeg avec libvmaf
./configure \
    --extra-ldflags="-L$DEPS_DIR/lib" \
    --extra-libs="-lstdc++ -lm -lpthread" \
    --enable-libvmaf
```

### 2. Pipeline d'encodage

Le pipeline utilise **uniquement ffmpeg** pour le muxing :

```rust
// ❌ NE PAS utiliser mkvmerge
Command::new("mkvmerge")...

// ✅ Utiliser ffmpeg
Command::new(&self.ffmpeg_bin)
    .arg("-i").arg(video_path)
    .arg("-i").arg(audio_path)
    .arg("-map").arg("0:v:0")
    .arg("-map").arg("1:a:0")
    ...
```

### 3. IPC entre daemon et TUI

- Communication via Unix socket : `~/.local/share/encodetalker/daemon.sock`
- Format : Bincode sérialisé avec `tokio-serde`
- Le daemon DOIT créer le socket AVANT de compiler les dépendances
- Les dépendances se compilent en arrière-plan (`tokio::spawn`)

### 4. Startup du daemon

**Ordre critique** :
1. Créer le socket Unix
2. Créer le pipeline et queue manager
3. Démarrer le serveur IPC
4. Compiler les dépendances en arrière-plan (si nécessaire)

```rust
// ✅ Bon ordre
let listener = tokio::net::UnixListener::bind(&socket_path)?;
let pipeline = EncodingPipeline::new(...);
let queue_manager = QueueManager::new(...);
ipc_server.run_with_listener(Some(listener), ...).await;

// En parallèle :
tokio::spawn(async { dep_manager.ensure_all_deps().await });
```

### 5. Gestion des processeurs modernes

GMP a des problèmes avec les processeurs récents (zen4). Si on devait réintroduire GMP :

```bash
# Utiliser --host=none pour éviter les tests stricts
./configure --prefix=... --host=none
```

## 🐛 Debugging

### Logs du daemon

```bash
# Voir les logs en temps réel
RUST_LOG=debug ./target/release/encodetalker-daemon 2>&1 | tee daemon.log

# Inspecter l'état persisté
cat ~/.local/share/encodetalker/state.json | jq

# Vérifier le socket
ls -la ~/.local/share/encodetalker/daemon.sock
```

### Processus

```bash
# Vérifier que le daemon tourne
ps aux | grep encodetalker-daemon

# Tuer le daemon
pkill -f encodetalker-daemon

# Nettoyer le socket
rm -f ~/.local/share/encodetalker/daemon.sock
```

### Dépendances

```bash
# Vérifier les binaires compilés
ls -lh ~/.local/share/encodetalker/deps/bin/

# Tester un binaire
~/.local/share/encodetalker/deps/bin/ffmpeg -version
~/.local/share/encodetalker/deps/bin/SvtAv1EncApp --help
```

## 🔄 Workflow Git

### Commits

Suivre le format Conventional Commits :

```
feat: ajouter support pour encodeur x265
fix: corriger crash lors de l'annulation d'un job
docs: mettre à jour README avec nouvelles options
refactor: simplifier le parser de stats ffmpeg
perf: optimiser la détection des dépendances
test: ajouter tests pour le pipeline d'encodage
chore: mettre à jour les dépendances Rust
```

### Avant de committer

```bash
# 1. Formatter le code
cargo fmt --all

# 2. Vérifier clippy (mode pedantic)
cargo clippy --all-targets --all-features -- -W clippy::pedantic

# 3. Lancer les tests
cargo test --all

# 4. Compiler en release pour vérifier
cargo build --release
```

## 📝 Ajout de fonctionnalités

### Ajouter un nouvel encodeur

1. Ajouter le builder dans `crates/encodetalker-deps/src/builder/`
2. Exporter dans `mod.rs` et `lib.rs`
3. Ajouter la détection dans `detector.rs`
4. Ajouter l'appel dans `manager.rs`
5. Mettre à jour le `EncodingPipeline`
6. Ajouter dans `EncoderType` (common)

### Ajouter une nouvelle vue TUI

1. Créer le fichier dans `crates/encodetalker-tui/src/ui/`
2. Ajouter le rendu dans `render.rs`
3. Ajouter la gestion des touches dans `input/handler.rs`
4. Ajouter le variant dans `View` enum

## 🚨 Ne JAMAIS faire

1. ❌ Ajouter des dépendances qui nécessitent sudo
2. ❌ Utiliser mkvmerge/mkvtoolnix (utiliser ffmpeg à la place)
3. ❌ Compiler les dépendances de manière synchrone avant de démarrer le serveur IPC
4. ❌ Utiliser `std::fs` pour les I/O (préférer `tokio::fs`)
5. ❌ Commit sans `cargo fmt` et sans corriger les warnings clippy
6. ❌ Utiliser `git add .` (toujours ajouter les fichiers spécifiquement)
7. ❌ Force push sur main/master

## 📚 Resources

- [Rust Async Book](https://rust-lang.github.io/async-book/)
- [Tokio Documentation](https://tokio.rs/)
- [Ratatui Book](https://ratatui.rs/)
- [FFmpeg Documentation](https://ffmpeg.org/documentation.html)
- [SVT-AV1 Guide](https://gitlab.com/AOMediaCodec/SVT-AV1)

## 🎯 Roadmap (futures fonctionnalités)

- [ ] Support d'autres encodeurs (x264, x265, VP9)
- [ ] Encodage multi-pass
- [ ] Filtres vidéo (crop, resize, denoise)
- [ ] Système de templates/presets
- [ ] API REST pour contrôle distant
- [ ] Interface web
- [x] Support Windows (✅ implémenté)
- [ ] Support macOS
- [ ] Notifications système
- [ ] Statistiques globales

## 💡 Tips

- **Performance** : Compiler en release pour les tests de performance
- **Debug** : Utiliser `RUST_LOG=trace` pour les logs très détaillés
- **Testing** : Tester avec différentes vidéos (courtes/longues, divers codecs)
- **IPC** : Utiliser `btop` pour voir l'utilisation CPU pendant l'encodage
- **Git** : Créer des commits atomiques (une fonctionnalité = un commit)

---

**Dernière mise à jour** : 2026-02-09
**Version** : 0.1.0
