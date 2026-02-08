# Guide de contribution

Merci de votre intérêt pour contribuer à EncodeTalker !

## Configuration de l'environnement de développement

### Prérequis

1. **Rust** : Version 1.70 ou supérieure
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Dépendances système** (pour compiler les dépendances) :
   ```bash
   # Sur Arch Linux / Manjaro
   sudo pacman -S base-devel cmake git nasm ruby libopus libvpx

   # Sur Ubuntu / Debian
   sudo apt install build-essential cmake git nasm ruby libopus-dev libvpx-dev
   ```

### Cloner et compiler

```bash
git clone https://github.com/votre-user/EncodeTalker.git
cd EncodeTalker

# Compiler en mode développement
cargo build

# Compiler en mode release
cargo build --release
```

## Standards de code

### Formatage

Avant de soumettre une PR, assurez-vous que le code est formaté :

```bash
cargo fmt --all
```

### Linting

Corrigez tous les warnings clippy :

```bash
cargo clippy --all-targets --all-features
```

### Tests

Lancez tous les tests :

```bash
cargo test --all
```

## Structure du projet

- `crates/encodetalker-common/` : Types communs et protocole IPC
- `crates/encodetalker-daemon/` : Daemon d'encodage
- `crates/encodetalker-tui/` : Interface TUI
- `crates/encodetalker-deps/` : Gestion des dépendances

## Guidelines de contribution

### Branches

- `main` : Branche stable
- `develop` : Branche de développement
- `feature/<nom>` : Nouvelles fonctionnalités
- `fix/<nom>` : Corrections de bugs

### Commits

Utilisez des messages de commit clairs et descriptifs :

```
feat: ajouter support pour encodeur x265
fix: corriger crash lors de l'annulation d'un job
docs: mettre à jour README avec nouvelles options
refactor: simplifier le parser de stats ffmpeg
```

Préfixes recommandés :
- `feat:` : Nouvelle fonctionnalité
- `fix:` : Correction de bug
- `docs:` : Documentation
- `refactor:` : Refactoring sans changement de comportement
- `test:` : Ajout/modification de tests
- `chore:` : Tâches de maintenance

### Pull Requests

1. **Créer une issue** décrivant le problème ou la fonctionnalité
2. **Fork le projet** et créer une branche
3. **Coder** en respectant les standards
4. **Tester** vos modifications
5. **Soumettre une PR** avec une description claire

### Code Review

Toutes les PR nécessitent :
- Code formaté (cargo fmt)
- Pas de warnings clippy
- Tests passants
- Documentation mise à jour si nécessaire

## Debugging

### Logs détaillés

```bash
# Daemon avec logs debug
RUST_LOG=debug cargo run --bin encodetalker-daemon

# TUI avec logs debug
RUST_LOG=debug cargo run --bin encodetalker-tui

# Logs très détaillés pour un module spécifique
RUST_LOG=encodetalker_daemon::encoder=trace cargo run --bin encodetalker-daemon
```

### Inspecter l'état

```bash
# État persisté du daemon
cat ~/.local/share/encodetalker/state.json | jq

# Vérifier le socket
ls -la ~/.local/share/encodetalker/daemon.sock

# Processus daemon
ps aux | grep encodetalker-daemon
```

## Roadmap

Consultez les [Issues](https://github.com/votre-user/EncodeTalker/issues) pour voir les tâches en cours et les fonctionnalités planifiées.

### Fonctionnalités prioritaires

- [ ] Tests d'intégration end-to-end
- [ ] Support encodeur x264/x265
- [ ] Sélection manuelle des streams audio/sous-titres
- [ ] Encodage multi-pass
- [ ] Filtres vidéo (crop, resize, denoise)

### Améliorations futures

- [ ] API REST pour contrôle distant
- [ ] Interface web
- [ ] Support macOS/Windows
- [ ] Notifications système
- [ ] Templates/presets d'encodage

## Questions ?

N'hésitez pas à :
- Ouvrir une [Issue](https://github.com/votre-user/EncodeTalker/issues)
- Rejoindre la discussion
- Demander de l'aide

Merci de contribuer à EncodeTalker ! 🎬
