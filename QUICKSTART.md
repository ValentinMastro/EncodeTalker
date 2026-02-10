# EncodeTalker - Guide de démarrage rapide

## Installation rapide

```bash
# 1. Compiler le projet
cargo build --release

# 2. Les binaires sont dans target/release/
ls target/release/encodetalker-*

# 3. Lancer l'interface TUI
./target/release/encodetalker-tui
```

## Première utilisation

Au premier lancement, le TUI va :
1. ✅ Démarrer automatiquement le daemon
2. ⏳ Compiler les dépendances (ffmpeg, SVT-AV1, etc.) - **30-60 minutes**
3. ✅ Se connecter et afficher l'interface

**Note importante :** La première fois, les dépendances (ffmpeg, SVT-AV1-psy, libaom, mkvtoolnix) seront téléchargées et compilées automatiquement. Cela peut prendre du temps !

## Dépendances système requises

Avant de commencer, installez :

```bash
# Sur Arch Linux / Manjaro
sudo pacman -S base-devel cmake git nasm ruby

# Les bibliothèques suivantes sont aussi utiles
sudo pacman -S libopus libvpx
```

## Utilisation rapide

### Interface TUI

```
┌─────────────────── EncodeTalker ────────────────────┐
│ [ Files ] [ Queue ] [ Active ] [ History ]          │
├─────────────────────────────────────────────────────┤
│                                                      │
│  📁 Videos/                                          │
│  ▶ 🎬 video1.mp4                                    │
│    🎬 video2.mkv                                    │
│    📁 subfolder/                                     │
│                                                      │
└─────────────────────────────────────────────────────┘
 Tab: Vue suivante | ↑↓: Naviguer | a: Ajouter | q: Quitter
```

### Workflow typique

1. **Naviguer** : Utiliser `↑↓` dans l'onglet Files
2. **Sélectionner** : Appuyer sur `a` sur une vidéo
3. **Configurer** :
   ```
   Encoder: SVT-AV1  [←→ pour changer]
   Audio:   Opus 128 kbps
   CRF:     30 (qualité)
   Preset:  6 (vitesse)
   ```
4. **Valider** : `Enter` pour ajouter à la queue
5. **Surveiller** : `Tab` pour voir la progression dans Active
6. **Résultat** : Le fichier .mkv sera créé à côté du fichier source

### Raccourcis essentiels

| Touche | Action |
|--------|--------|
| `Tab` | Changer d'onglet |
| `↑↓` ou `j`/`k` | Naviguer |
| `Enter` | Ouvrir / Sélectionner |
| `a` | Ajouter à la queue |
| `c` | Annuler un job |
| `r` | Rafraîchir / Retry |
| `q` | Quitter |

## Configuration des encodages

### Encodeur

- **SVT-AV1** (recommandé) : Rapide, excellente qualité
- **libaom AV1** : Plus lent, qualité légèrement meilleure

### CRF (Qualité)

- `18-24` : Très haute qualité (gros fichiers)
- `28-32` : Bonne qualité (équilibré) ⭐ Recommandé
- `35-40` : Qualité acceptable (petits fichiers)

### Preset (Vitesse)

**SVT-AV1** (0-13) :
- `4-6` : Bon équilibre vitesse/qualité ⭐ Recommandé
- `8-10` : Plus rapide, qualité légèrement réduite
- `12-13` : Très rapide, pour tests

**libaom** (0-8) :
- `4` : Équilibré ⭐ Recommandé
- `6` : Plus rapide
- `8` : Très rapide, qualité réduite

### Mode audio

- **Opus 128k** : Encodage audio en Opus (recommandé)
- **Copy** : Copie directe sans ré-encodage

## Exemple de résultats

```
Input:  video.mp4 (1.2 GB, H.264)
Config: SVT-AV1, CRF 30, Preset 6, Opus 128k
Output: video.av1.mkv (450 MB)
Ratio:  ~2.7x de compression
Time:   ~30 min (dépend du CPU)
```

## Fichiers créés

```
~/.local/share/encodetalker/
├── deps/
│   ├── bin/          # Binaires compilés
│   │   ├── ffmpeg
│   │   ├── ffprobe
│   │   ├── SvtAv1EncApp
│   │   ├── aomenc
│   │   └── mkvmerge
│   └── src/          # Sources téléchargées
├── state.json        # État du daemon (queue, jobs)
├── daemon.sock       # Socket IPC
└── daemon.log        # Logs

~/.config/encodetalker/
└── config.toml       # Configuration utilisateur
```

## Débogage

### Le TUI ne démarre pas

```bash
# Vérifier que le daemon peut démarrer
./target/release/encodetalker-daemon

# Vérifier les logs
cat ~/.local/share/encodetalker/daemon.log
```

### Jobs échouent

1. Vérifier dans l'onglet History le message d'erreur
2. Consulter les logs du daemon
3. S'assurer que les dépendances sont bien compilées :
   ```bash
   ls -la ~/.local/share/encodetalker/deps/bin/
   ```

### Reconnexion au daemon

Le daemon tourne en arrière-plan. Vous pouvez :
- Fermer le TUI avec `q`
- Relancer plus tard : `./target/release/encodetalker-tui`
- Les jobs continuent pendant ce temps !

## Arrêter proprement

```bash
# Quitter le TUI
# Appuyer sur 'q'

# Le daemon continue en arrière-plan
# Pour l'arrêter :
pkill -f encodetalker-daemon

# Ou envoyer SIGTERM
kill $(pgrep -f encodetalker-daemon)
```

## Support

- README complet : `README.md`
- Issues : https://github.com/anthropics/claude-code/issues
- Logs daemon : `~/.local/share/encodetalker/daemon.log`

## Conseils

✅ **À faire** :
- Tester avec une petite vidéo d'abord
- Utiliser CRF 30 et Preset 6 pour commencer
- Laisser le daemon tourner en arrière-plan
- Fermer le TUI sans problème, il se reconnecte

❌ **À éviter** :
- Ne pas killer le daemon pendant un encodage (utiliser 'c' dans le TUI)
- Ne pas encoder sur des vidéos déjà en AV1 (redondant)
- Ne pas utiliser CRF trop bas (<20) sauf si vraiment nécessaire

## Prochaines étapes

Une fois familiarisé :
1. Ajuster la configuration dans `~/.config/encodetalker/config.toml`
2. Augmenter `max_concurrent_jobs` si vous avez un CPU puissant
3. Personnaliser les presets d'encodage
4. Explorer les jobs terminés dans History

Bon encodage ! 🎬
