# EncodeTalker

Un wrapper Rust autour de ffmpeg avec interface TUI pour gérer l'encodage vidéo en AV1.

## Caractéristiques

- **Architecture client-serveur** : Le daemon gère l'encodage en arrière-plan, le TUI est reconnectable
- **Persistance** : Les jobs d'encodage continuent même si vous fermez le TUI
- **Queue intelligente** : Gestion automatique de la queue avec concurrence configurable
- **Compilation automatique des dépendances** : ffmpeg, SVT-AV1-psy, libaom
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
- Compilation de ffmpeg, SVT-AV1-psy, libaom
- Installation dans `~/.local/share/encodetalker/deps/`

### ✅ Phase 3 : Daemon (Complète)
- Queue manager avec concurrence configurable
- Pipeline d'encodage (ffmpeg → encodeur → ffmpeg muxing)
- Parser de stats ffmpeg en temps réel
- Serveur IPC Unix socket avec broadcast d'événements
- Persistance de l'état (JSON)
- Auto-save toutes les 10 secondes
- Shutdown graceful

### ✅ Phase 4 : TUI (Complète)
- Interface ratatui avec 4 vues principales
- **FileBrowser** : Navigation filesystem avec filtrage des vidéos
- **QueueView** : Liste des jobs en attente
- **ActiveView** : Jobs en cours avec stats temps réel et barres de progression
- **HistoryView** : Historique avec retry des jobs failed
- Client IPC avec reconnexion automatique
- Démarrage automatique du daemon
- Dialogues interactifs :
  * Configuration d'encodage (encoder, audio, CRF, preset)
  * Confirmation (annulation, clear history)
  * Affichage d'erreurs
- Gestion clavier complète (vim-like + flèches)
- Rafraîchissement automatique toutes les 500ms
- Événements temps réel du daemon (progression, completion)

### ✅ Phase 5 : Polish (Complète)
- Code formaté avec `cargo fmt`
- Tous les warnings clippy corrigés
- Documentation complète dans README.md
- Logs détaillés avec tracing
- Gestion robuste des erreurs

## Compilation

```bash
# Compiler tout le workspace
cargo build --release

# Compiler uniquement le daemon
cargo build --release -p encodetalker-daemon
```

## Utilisation

### Lancement du TUI (recommandé)

```bash
./target/release/encodetalker-tui
```

Le TUI va :
1. Vérifier si le daemon est en cours d'exécution
2. Démarrer automatiquement le daemon si nécessaire
3. Se connecter au daemon via IPC
4. Afficher l'interface interactive

**Navigation :**
- `Tab` : Changer de vue (Files → Queue → Active → History)
- `↑↓` ou `k`/`j` : Naviguer dans les listes
- `Enter` : Ouvrir un répertoire ou configurer un fichier vidéo
- `a` : Ajouter une vidéo à la queue (dans Files)
- `c` : Annuler un job (dans Queue/Active)
- `r` : Rafraîchir ou Retry un job failed (dans History)
- `C` : Clear l'historique (dans History)
- `q` : Quitter

### Démarrage manuel du daemon (optionnel)

```bash
./target/release/encodetalker-daemon
```

Le daemon va :
1. Vérifier les dépendances dans `~/.local/share/encodetalker/deps/bin/`
2. Compiler les dépendances manquantes (peut prendre 30-60 minutes la première fois)
3. Écouter sur le socket Unix : `~/.local/share/encodetalker/daemon.sock`
4. Charger l'état sauvegardé s'il existe

### Dépendances système requises

Pour compiler les dépendances (ffmpeg, SVT-AV1, libaom), vous devez installer :

```bash
# Sur Arch Linux / Manjaro
sudo pacman -S base-devel cmake git nasm

# Sur Ubuntu / Debian
sudo apt install build-essential cmake git nasm

# Sur Fedora
sudo dnf install @development-tools cmake git nasm
```

**Note** : Toutes les dépendances sont compilées localement sans nécessiter d'accès sudo ! 🎉

⏱️ **Temps de compilation estimé** :
- FFmpeg : 15-20 min
- SVT-AV1 : 10-15 min
- libaom : 15-20 min

**Total : ~40-55 minutes la première fois**

**Muxing** : ffmpeg est utilisé pour créer les fichiers MKV finaux (pas besoin de mkvtoolnix)

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
  - `deps/bin/` : Binaires compilés (ffmpeg, ffprobe, SvtAv1EncApp, aomenc)
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
    ffmpeg (mux vidéo.ivf + audio.opus + subtitles) → fichier.mkv
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

## Workflow typique

1. **Lancer le TUI** : `./target/release/encodetalker-tui`
2. **Naviguer vers vos vidéos** : Utiliser `↑↓` et `Enter` dans l'onglet Files
3. **Sélectionner une vidéo** : Appuyer sur `a` ou `Enter` sur un fichier vidéo
4. **Configurer l'encodage** :
   - Choisir l'encodeur (SVT-AV1 ou libaom)
   - Configurer l'audio (Opus ou Copy)
   - Ajuster CRF (qualité) et Preset (vitesse)
   - Valider avec `Enter`
5. **Surveiller la progression** : Basculer vers l'onglet Active (Tab)
6. **Vérifier les résultats** : Consulter l'historique dans l'onglet History

Le TUI se reconnecte automatiquement au daemon, vous pouvez le fermer et revenir plus tard !

## Raccourcis clavier

### Globaux
- `Tab` : Vue suivante
- `Shift+Tab` : Vue précédente
- `q` : Quitter

### File Browser
- `↑↓` / `k`/`j` : Naviguer
- `Enter` : Ouvrir répertoire ou configurer vidéo
- `a` : Ajouter à la queue
- `r` : Rafraîchir

### Queue & Active
- `↑↓` / `k`/`j` : Naviguer
- `c` : Annuler le job sélectionné
- `r` : Rafraîchir

### History
- `↑↓` / `k`/`j` : Naviguer
- `r` : Retry un job failed
- `Shift+C` : Clear tout l'historique

### Dialogues
- `↑↓` : Naviguer entre les champs
- `←→` : Modifier la valeur
- `Enter` : Valider
- `ESC` : Annuler

## Limitations actuelles

- Sélection manuelle des streams audio/sous-titres non implémentée (tous inclus par défaut)
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
