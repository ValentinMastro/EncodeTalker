# Guide de migration : Chemins personnalisés

## 🎯 Nouveauté : Chemins configurables (v0.1.0+)

EncodeTalker supporte désormais la personnalisation des chemins via `config.toml`. Cette fonctionnalité permet de :

- ✅ Déplacer les dépendances (~500 MB) sur un SSD pour compilation plus rapide
- ✅ Utiliser un socket personnalisé pour scénarios multi-utilisateurs
- ✅ Déplacer toutes les données sur un disque externe
- ✅ Rétrocompatibilité totale : aucun changement requis pour installations existantes

## 📋 Compatibilité

**Anciens utilisateurs** : Aucune action requise ! Si votre `config.toml` n'a pas de section `[paths]`, le comportement reste identique :
- Données : `~/.local/share/encodetalker/`
- Dépendances : `~/.local/share/encodetalker/deps/`
- Socket : `~/.local/share/encodetalker/daemon.sock`

**Nouveaux utilisateurs** : Vous pouvez configurer les chemins dès l'installation.

## 🚀 Cas d'usage

### 1. Déplacer uniquement les dépendances sur SSD

**Problème** : Les dépendances (~500 MB) se compilent lentement sur HDD.
**Solution** : Déplacer uniquement `deps_dir` sur SSD.

```toml
[paths]
deps_dir = "/mnt/ssd/encodetalker-deps"
```

**Résultat** :
- Dépendances : `/mnt/ssd/encodetalker-deps/` (SSD rapide)
- Données : `~/.local/share/encodetalker/` (HDD, inchangé)
- Socket : `~/.local/share/encodetalker/daemon.sock` (inchangé)

### 2. Multi-utilisateurs : socket personnalisé

**Problème** : Plusieurs utilisateurs veulent lancer leur propre daemon.
**Solution** : Socket avec variable `$USER`.

```toml
[paths]
socket_path = "/tmp/encodetalker-$USER.sock"
```

**Résultat** :
- Utilisateur `alice` : `/tmp/encodetalker-alice.sock`
- Utilisateur `bob` : `/tmp/encodetalker-bob.sock`
- Chaque utilisateur a son propre daemon indépendant

### 3. Tout déplacer sur disque externe

**Problème** : Partition home limitée, grand disque externe disponible.
**Solution** : Déplacer `data_dir` complet.

```toml
[paths]
data_dir = "/mnt/external/encodetalker"
```

**Résultat** :
- Toutes les données : `/mnt/external/encodetalker/`
- Dépendances : `/mnt/external/encodetalker/deps/` (dérivé)
- Socket : `/mnt/external/encodetalker/daemon.sock` (dérivé)

### 4. Configuration granulaire

**Besoin** : SSD pour deps, HDD pour données, socket dans /tmp.

```toml
[paths]
data_dir = "/mnt/hdd/encodetalker-data"
deps_dir = "/mnt/ssd/encodetalker-deps"
socket_path = "/tmp/encodetalker.sock"
```

## 🔧 Migration pas à pas

### Migrer des dépendances existantes

Si vous avez déjà compilé les dépendances et voulez les déplacer :

```bash
# 1. Arrêter le daemon
pkill -f encodetalker-daemon

# 2. Déplacer les dépendances
mv ~/.local/share/encodetalker/deps /mnt/ssd/encodetalker-deps

# 3. Configurer le nouveau chemin
cat >> ~/.config/encodetalker/config.toml << 'EOF'

[paths]
deps_dir = "/mnt/ssd/encodetalker-deps"
EOF

# 4. Redémarrer le daemon
./target/release/encodetalker-daemon
```

### Migrer toutes les données

```bash
# 1. Arrêter le daemon
pkill -f encodetalker-daemon

# 2. Déplacer les données
mv ~/.local/share/encodetalker /mnt/external/

# 3. Configurer le nouveau chemin
cat >> ~/.config/encodetalker/config.toml << 'EOF'

[paths]
data_dir = "/mnt/external/encodetalker"
EOF

# 4. Redémarrer
./target/release/encodetalker-daemon
```

## 🛡️ Garanties et limitations

### Garanties

✅ **Rétrocompatibilité** : Anciennes installations fonctionnent sans modification
✅ **Cohérence daemon/TUI** : Les deux lisent le même `config.toml`
✅ **Expansion de chemins** : Support de `~` et variables d'environnement
✅ **Validation** : Erreurs claires si chemin invalide ou sans permission

### Limitations

❌ **config_dir non configurable** : Toujours `~/.config/encodetalker/` (pour éviter confusion)
❌ **Pas de rechargement à chaud** : Redémarrer le daemon après modification
❌ **Chemins absolus recommandés** : Éviter les chemins relatifs

## 🐛 Dépannage

### Le daemon ne démarre pas après changement de chemins

**Symptôme** : Erreur "Permission denied" ou "No such file or directory"

**Solution** :
1. Vérifier que le chemin existe et est accessible
2. Créer manuellement les répertoires si nécessaire
3. Vérifier les permissions (doit être accessible en lecture/écriture)

```bash
# Créer répertoire si nécessaire
mkdir -p /mnt/ssd/encodetalker-deps

# Vérifier permissions
ls -ld /mnt/ssd/encodetalker-deps
```

### Le TUI ne se connecte pas au daemon

**Symptôme** : "Cannot connect to daemon socket"

**Causes possibles** :
1. Daemon et TUI lisent des configs différentes (impossible si même utilisateur)
2. Variable d'environnement différente (ex: `$USER` changé)
3. Daemon pas redémarré après changement de config

**Solution** :
```bash
# 1. Vérifier que daemon utilise bon socket
ps aux | grep encodetalker-daemon
cat ~/.config/encodetalker/config.toml | grep socket_path

# 2. Redémarrer daemon
pkill -f encodetalker-daemon
./target/release/encodetalker-daemon
```

### Variable d'environnement non expansée

**Symptôme** : Chemin contient littéralement `$USER` au lieu du nom

**Cause** : Variable inexistante ou non définie

**Solution** :
```bash
# Vérifier que variable existe
echo $USER

# Utiliser ~ si HOME disponible
[paths]
data_dir = "~/encodetalker-data"
```

## 📚 Références

- [README.md](README.md) : Documentation principale
- [config/config.toml](config/config.toml) : Configuration par défaut avec exemples
- [CLAUDE.md](CLAUDE.md) : Documentation technique pour développeurs

## 💡 Astuces

1. **Tester avec chemins temporaires** : Utilisez `/tmp/encodetalker-test` pour tester sans affecter installation
2. **Backup avant migration** : Copiez `~/.local/share/encodetalker` avant de déplacer
3. **Logs pour debug** : `RUST_LOG=debug ./target/release/encodetalker-daemon` montre les chemins utilisés
4. **Symlinks fonctionnent** : Vous pouvez créer un lien symbolique au lieu de configurer
