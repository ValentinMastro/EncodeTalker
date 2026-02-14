# Cross-compilation pour Windows

Ce document explique comment compiler EncodeTalker pour Windows depuis Linux.

## Prérequis

### Sur Manjaro/Arch Linux

```bash
# Installer MinGW-w64 toolchain
sudo pacman -S mingw-w64-gcc mingw-w64-binutils mingw-w64-crt mingw-w64-headers mingw-w64-winpthreads

# Ajouter le target Windows à Rust
rustup target add x86_64-pc-windows-gnu
```

### Sur Ubuntu/Debian

```bash
# Installer MinGW-w64
sudo apt install gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64

# Ajouter le target Windows à Rust
rustup target add x86_64-pc-windows-gnu
```

## Compilation

### Méthode simple (recommandée)

Utilisez le script fourni :

```bash
./build-windows.sh
```

### Méthode manuelle

```bash
cargo build --release --target x86_64-pc-windows-gnu
```

Les binaires seront dans `target/x86_64-pc-windows-gnu/release/` :
- `encodetalker-daemon.exe`
- `encodetalker-tui.exe`

## Vérification

### Test de compilation uniquement

Pour vérifier que le code compile sans produire les binaires :

```bash
cargo check --target x86_64-pc-windows-gnu
```

### Cross-compilation des tests

```bash
cargo test --target x86_64-pc-windows-gnu --no-run
```

**Note** : Les tests ne peuvent pas s'exécuter sur Linux, uniquement compiler.

## Distribution Windows

### Structure de distribution

Créez un dossier avec :

```
encodetalker-windows/
├── encodetalker-daemon.exe
├── encodetalker-tui.exe
├── README.txt
└── LICENSE
```

### Dépendances runtime

Les binaires compilés avec MinGW nécessitent ces DLLs (généralement déjà présentes sur Windows 10/11) :
- `msvcrt.dll` (C Runtime)
- `kernel32.dll` (Windows API)
- `ws2_32.dll` (Windows Sockets)

Ces DLLs sont normalement déjà présentes sur Windows. Si ce n'est pas le cas, elles sont disponibles dans MinGW :
```bash
/usr/x86_64-w64-mingw32/bin/libgcc_s_seh-1.dll
/usr/x86_64-w64-mingw32/bin/libwinpthread-1.dll
```

### FFmpeg et encodeurs

Sur Windows, le daemon téléchargera automatiquement les binaires pré-compilés :
- **FFmpeg** : Téléchargé depuis GitHub (BtbN/FFmpeg-Builds)
- **SVT-AV1** : À implémenter (prévu dans le plan)
- **aomenc** : À implémenter (prévu dans le plan)

Les binaires seront placés dans `%LOCALAPPDATA%\encodetalker\deps\bin\`

## Test sur Windows

1. **Copiez les binaires** sur une machine Windows (ou VM)

2. **Lancez le TUI** :
   ```cmd
   encodetalker-tui.exe
   ```

3. **Le daemon se lancera automatiquement** en arrière-plan

4. **Vérifiez les logs** (si problème) :
   ```
   %LOCALAPPDATA%\encodetalker\daemon.log
   ```

## Alternatives

### Option 1 : Compiler directement sur Windows

Si vous avez accès à une machine Windows :

1. Installez Rust : https://rustup.rs/
2. Installez Git et CMake
3. Clonez le dépôt
4. Compilez :
   ```cmd
   cargo build --release
   ```

### Option 2 : Cross-compilation avec Docker

```bash
# TODO: Créer un Dockerfile pour cross-compilation
docker build -t encodetalker-builder .
docker run --rm -v $(pwd):/workspace encodetalker-builder
```

## Dépannage

### Erreur : "linker `x86_64-w64-mingw32-gcc` not found"

Solution : Vérifiez que MinGW est installé et dans le PATH :
```bash
which x86_64-w64-mingw32-gcc
```

Si absent, réinstallez :
```bash
sudo pacman -S mingw-w64-gcc
```

### Erreur : "target 'x86_64-pc-windows-gnu' not found"

Solution : Ajoutez le target :
```bash
rustup target add x86_64-pc-windows-gnu
```

### Erreur de linkage avec tokio/winapi

Vérifiez que `.cargo/config.toml` contient :
```toml
[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"
ar = "x86_64-w64-mingw32-ar"
```

### Les binaires ne fonctionnent pas sur Windows

- Vérifiez l'architecture (64-bit uniquement)
- Vérifiez que les DLLs MinGW sont présentes
- Consultez les logs dans `%LOCALAPPDATA%\encodetalker\daemon.log`

## Limitations

### Actuelles

1. **Dépendances pré-compilées** : Seul FFmpeg est implémenté
   - SVT-AV1 : Prévu (Phase 3.6 du plan)
   - aomenc : Prévu (Phase 3.6 du plan)

2. **Tests fonctionnels** : Impossible d'exécuter les tests cross-compilés sur Linux
   - Solution : Tester sur vraie machine Windows

3. **Named Pipes** : Non testés en conditions réelles
   - Solution : Test fonctionnel sur Windows requis

### Prévues (à implémenter)

- [ ] Builder pré-compilé pour SVT-AV1 Windows
- [ ] Builder pré-compilé pour aomenc Windows
- [ ] Tests d'intégration Windows (via GitHub Actions)
- [ ] Package MSI pour installation Windows
- [ ] Documentation utilisateur Windows

## CI/CD

Pour automatiser la cross-compilation dans GitHub Actions :

```yaml
# .github/workflows/cross-compile.yml
name: Cross-compile Windows

on: [push, pull_request]

jobs:
  build-windows:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: x86_64-pc-windows-gnu
      - name: Install MinGW
        run: sudo apt-get install -y gcc-mingw-w64-x86-64
      - name: Build
        run: cargo build --release --target x86_64-pc-windows-gnu
      - name: Upload artifacts
        uses: actions/upload-artifact@v3
        with:
          name: windows-binaries
          path: |
            target/x86_64-pc-windows-gnu/release/encodetalker-daemon.exe
            target/x86_64-pc-windows-gnu/release/encodetalker-tui.exe
```

## Ressources

- [Rust Cross-compilation Guide](https://rust-lang.github.io/rustup/cross-compilation.html)
- [MinGW-w64 Project](https://www.mingw-w64.org/)
- [Tokio on Windows](https://tokio.rs/tokio/topics/bridging)
- [Windows Named Pipes](https://docs.microsoft.com/en-us/windows/win32/ipc/named-pipes)

---

**Dernière mise à jour** : 2026-02-14
**Statut** : ✅ Compilation fonctionnelle | ⏳ Tests Windows en attente
