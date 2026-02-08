# EncodeTalker

Un wrapper Rust autour de ffmpeg avec interface TUI pour gérer l'encodage vidéo en AV1.

## Caractéristiques

- **Architecture client-serveur** : Le daemon gère l'encodage en arrière-plan, le TUI est reconnectable
- **Persistance** : Les jobs d'encodage continuent même si vous fermez le TUI
- **Queue intelligente** : Gestion automatique de la queue avec concurrence configurable
- **Compilation automatique des dépendances** : ffmpeg, SVT-AV1-psy, libaom, mkvtoolnix
- **Suivi temps réel** : Progression, FPS, ETA pour chaque encodage
- **Pipeline flexible** : Support SVT-AV1 et libaom, audio Opus ou copy, sous-titres

## Structure du projet

```
EncodeTalker/
├── crates/
│   ├── encodetalker-common/     # Types communs et protocole IPC
│   ├── encodetalker-daemon/     # Daemon d'encodage
│   ├── encodetalker-tui/        # Interface TUI (TODO)
│   └── encodetalker-deps/       # Gestion des dépendances
└── config/
    └── default.toml             # Configuration par défaut
```

## État d'implémentation

### ✅ Phase 1 : Infrastructure (Complète)
- Types communs (Job, Status, Stats, Config)
- Protocole IPC (Request/Response/Event)
- Gestion des chemins de l'application

### ✅ Phase 2 : Gestion des dépendances (Complète)
- Détection automatique des binaires
- Téléchargement des sources
- Compilation de ffmpeg, SVT-AV1-psy, libaom, mkvtoolnix
- Installation dans `~/.local/share/encodetalker/deps/`

### ✅ Phase 3 : Daemon (Complète)
- Queue manager avec concurrence configurable
- Pipeline d'encodage (ffmpeg → encodeur → mkvmerge)
- Parser de stats ffmpeg en temps réel
- Serveur IPC Unix socket avec broadcast d'événements
- Persistance de l'état (JSON)
- Auto-save toutes les 10 secondes
- Shutdown graceful

### ⏳ Phase 4 : TUI (TODO)
- Interface ratatui
- Navigation filesystem
- Vues : FileBrowser, QueueView, ActiveView, HistoryView
- Client IPC
- Dialogues de configuration

### ⏳ Phase 5 : Polish (TODO)
- Tests d'intégration
- Documentation utilisateur
- Gestion robuste des erreurs

## Compilation

```bash
# Compiler tout le workspace
cargo build --release

# Compiler uniquement le daemon
cargo build --release -p encodetalker-daemon
```

## Utilisation

### Démarrage du daemon

```bash
./target/release/encodetalker-daemon
```

Le daemon va :
1. Vérifier les dépendances dans `~/.local/share/encodetalker/deps/bin/`
2. Compiler les dépendances manquantes (peut prendre 30-60 minutes la première fois)
3. Écouter sur le socket Unix : `~/.local/share/encodetalker/daemon.sock`
4. Charger l'état sauvegardé s'il existe

### Dépendances système requises

Pour compiler les dépendances, vous devez avoir :

```bash
# Sur Arch Linux / Manjaro
sudo pacman -S base-devel cmake git nasm ruby

# Les dépendances suivantes peuvent également être nécessaires
sudo pacman -S libopus libvpx
```

## Configuration

Le fichier de configuration est `~/.config/encodetalker/config.toml` :

```toml
[daemon]
max_concurrent_jobs = 1  # Nombre de jobs simultanés
socket_path = "~/.local/share/encodetalker/daemon.sock"
log_level = "info"

[encoding]
default_encoder = "svt-av1"
default_audio_mode = "opus"
default_audio_bitrate = 128
output_suffix = ".av1"

[encoder.svt-av1]
preset = 6     # 0-13, plus élevé = plus rapide
crf = 30       # 0-63, plus bas = meilleure qualité
params = ["--keyint", "240", "--tune", "3"]

[encoder.aom]
cpu-used = 4   # 0-8, plus élevé = plus rapide
crf = 30

[ui]
file_extensions = [".mp4", ".mkv", ".avi", ".mov", ".webm"]
refresh_interval_ms = 500
```

## Fichiers créés

- `~/.local/share/encodetalker/` : Répertoire de données
  - `deps/bin/` : Binaires compilés (ffmpeg, SvtAv1EncApp, aomenc, mkvmerge)
  - `deps/src/` : Sources téléchargées
  - `state.json` : État persisté (queue, active jobs, history)
  - `daemon.sock` : Socket Unix pour IPC
  - `daemon.log` : Logs du daemon
- `~/.config/encodetalker/` : Configuration
  - `config.toml` : Configuration utilisateur

## Architecture technique

### Pipeline d'encodage

```
fichier.mp4
    │
    ├─→ ffmpeg (demux + raw video en yuv4mpegpipe)
    │       │
    │       ↓ stdout
    │   Encodeur (SVT-AV1 ou libaom)
    │       │
    │       ↓ output
    │   fichier.ivf (vidéo AV1)
    │
    └─→ ffmpeg (extract audio)
            │
            ↓
        audio.opus (ou copy)

Ensuite:
    mkvmerge (mux vidéo.ivf + audio.opus + subtitles) → fichier.mkv
```

### Protocole IPC

Communication via Unix socket avec messages bincode sérialisés :

- **Requests** : AddJob, CancelJob, RetryJob, ListQueue, ListActive, ListHistory, etc.
- **Responses** : Ok, Error, Job, JobList, Stats, etc.
- **Events** (broadcast) : JobAdded, JobStarted, JobProgress, JobCompleted, JobFailed, JobCancelled

### Gestion de la queue

- Jobs stockés dans une `VecDeque<EncodingJob>`
- Jobs actifs dans `HashMap<Uuid, EncodingJob>`
- Historique dans `Vec<EncodingJob>`
- Démarrage automatique quand des slots se libèrent
- Notified pattern avec `tokio::sync::Notify` pour éviter les boucles actives

## Limitations actuelles

- Pas de TUI (Phase 4 non implémentée)
- Pas d'API pour interagir avec le daemon (sauf via le socket Unix)
- Un seul job simultané par défaut (configurable)
- Pipeline audio simplifié (Opus ou copy uniquement)

## Prochaines étapes

1. Implémenter le TUI avec ratatui
2. Ajouter des tests d'intégration
3. Améliorer la gestion des erreurs
4. Support de plus d'encodeurs (x264, x265, VP9)
5. Encodage multi-pass
6. Filtres vidéo (crop, resize, denoise)

## Licence

MIT OR Apache-2.0

## Auteurs

EncodeTalker Team
