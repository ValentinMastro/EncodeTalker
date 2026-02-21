# Makefile pour EncodeTalker
# Facilite la compilation, le nettoyage et le lancement du projet

.PHONY: all build build-dev static test test-unit test-integration clean clean-all fmt clippy run-daemon run-tui stop install help

# Variables
CARGO := cargo
INSTALL_DIR := $(HOME)/.local/bin
DATA_DIR := $(HOME)/.local/share/encodetalker
CONFIG_DIR := $(HOME)/.config/encodetalker

# Target par défaut
all: build

# Aide
help:
	@echo "EncodeTalker - Targets disponibles:"
	@echo ""
	@echo "  make build       - Compiler en mode release"
	@echo "  make build-dev   - Compiler en mode développement"
	@echo "  make static      - Compiler en statique (musl, portable)"
	@echo "  make test        - Lancer tous les tests"
	@echo "  make test-unit   - Lancer les tests unitaires (rapides)"
	@echo "  make test-integration - Lancer les tests d'intégration (nécessite vidéo)"
	@echo "  make clean       - Nettoyer le build Cargo + dépendances compilées"
	@echo "  make clean-all   - Nettoyer tout (build + dépendances + socket)"
	@echo "  make fmt         - Formatter le code"
	@echo "  make clippy      - Linter avec clippy"
	@echo "  make check       - Vérifier (fmt + clippy + test)"
	@echo "  make run-daemon  - Lancer le daemon avec logs"
	@echo "  make run-tui     - Lancer le TUI"
	@echo "  make stop        - Arrêter le daemon"
	@echo "  make install     - Installer les binaires dans ~/.local/bin"
	@echo "  make uninstall   - Désinstaller les binaires"
	@echo "  make help        - Afficher cette aide"
	@echo ""

# Compilation
build:
	@echo "🔨 Compilation en mode release..."
	$(CARGO) build --release

build-dev:
	@echo "🔨 Compilation en mode développement..."
	$(CARGO) build

# Compilation statique (portable, compatible toutes distributions Linux x86_64)
static:
	@echo "🔨 Compilation statique avec musl..."
	@echo "   (Binaire portable, fonctionne sur toutes les distributions Linux)"
	@echo ""
	@echo "Vérification des dépendances musl..."
	@if ! command -v musl-gcc >/dev/null 2>&1; then \
		echo "❌ musl-gcc n'est pas installé"; \
		echo "   Installez avec: sudo pacman -S musl rust-musl (Arch/Manjaro)"; \
		echo "               ou: sudo apt install musl-tools (Ubuntu/Debian)"; \
		exit 1; \
	fi
	@if ! rustc --print target-list 2>/dev/null | grep -q "x86_64-unknown-linux-musl"; then \
		echo "❌ La target musl n'est pas disponible"; \
		echo "   Installez avec: sudo pacman -S rust-musl (Arch/Manjaro)"; \
		echo "               ou: rustup target add x86_64-unknown-linux-musl (rustup)"; \
		exit 1; \
	fi
	@if ! ls /usr/lib/rustlib/x86_64-unknown-linux-musl/lib/libstd-*.rlib >/dev/null 2>&1; then \
		echo "❌ Les bibliothèques Rust musl ne sont pas installées"; \
		echo "   Installez avec: sudo pacman -S rust-musl"; \
		exit 1; \
	fi
	@echo "✅ Toutes les dépendances musl sont présentes"
	@echo ""
	$(CARGO) build --release --target x86_64-unknown-linux-musl
	@echo ""
	@echo "✅ Compilation statique terminée"
	@echo ""
	@echo "📦 Binaires portables disponibles dans:"
	@echo "  target/x86_64-unknown-linux-musl/release/encodetalker-daemon"
	@echo "  target/x86_64-unknown-linux-musl/release/encodetalker-tui"
	@echo ""
	@echo "Ces binaires fonctionnent sur TOUTES les distributions Linux x86_64"
	@echo "sans dépendances dynamiques (pas de problème de version glibc)"
	@echo ""
	@ls -lh target/x86_64-unknown-linux-musl/release/encodetalker-{daemon,tui} 2>/dev/null || true

# Tests
test:
	@echo "🧪 Lancement des tests..."
	$(CARGO) test --all

# Tests unitaires (rapides)
test-unit:
	@echo "🧪 Tests unitaires..."
	$(CARGO) test --all --lib

# Tests d'intégration (nécessite vidéo test)
test-integration:
	@echo "🧪 Lancement des tests d'intégration..."
	@if [ ! -f "vidéos_de_test/test1.mkv" ]; then \
		echo "❌ Vidéo de test manquante: vidéos_de_test/test1.mkv"; \
		exit 1; \
	fi
	RUST_LOG=info $(CARGO) test -p encodetalker-daemon --test integration_tests -- --ignored --nocapture

# Formatage et linting
fmt:
	@echo "✨ Formatage du code..."
	$(CARGO) fmt --all

clippy:
	@echo "🔍 Linting avec clippy..."
	$(CARGO) clippy --all-targets --all-features

# Vérification complète
check: fmt clippy test
	@echo "✅ Vérification complète terminée"

# Nettoyage
clean:
	@echo "🧹 Nettoyage du build Cargo..."
	$(CARGO) clean
	@echo "🧹 Suppression des dépendances compilées..."
	@if [ -d "$(DATA_DIR)/deps" ]; then \
		echo "   Suppression de $(DATA_DIR)/deps/"; \
		rm -rf "$(DATA_DIR)/deps"; \
	fi
	@echo "✅ Nettoyage terminé"

# Nettoyage complet (tout supprimer)
clean-all: clean
	@echo "🧹 Suppression de toutes les données..."
	@if [ -d "$(DATA_DIR)" ]; then \
		echo "   Suppression de $(DATA_DIR)/"; \
		rm -rf "$(DATA_DIR)"; \
	fi
	@echo "🧹 Suppression des dépendances locales (.dependencies/)..."
	@if [ -d ".dependencies" ]; then \
		echo "   Suppression de .dependencies/"; \
		rm -rf ".dependencies"; \
	fi
	@echo "🧹 Suppression des fichiers .log..."
	@find . -name "*.log" -type f -delete 2>/dev/null || true
	@echo "🧹 Arrêt du daemon si en cours..."
	@pgrep -f "encodetalker-daemon$$" | xargs -r kill 2>/dev/null || true
	@echo "✅ Nettoyage complet terminé"

# Lancement
run-daemon:
	@echo "🚀 Lancement du daemon..."
	@if pgrep -f encodetalker-daemon > /dev/null; then \
		echo "⚠️  Le daemon est déjà en cours d'exécution"; \
		echo "   Arrêtez-le avec: pkill -f encodetalker-daemon"; \
		exit 1; \
	fi
	RUST_LOG=info ./target/release/encodetalker-daemon

run-tui:
	@echo "🖥️  Lancement du TUI..."
	./target/release/encodetalker-tui

stop:
	@echo "🛑 Arrêt du daemon..."
	@pgrep -f "target/release/encodetalker-daemon$$" | xargs -r kill 2>/dev/null || true
	@echo "✅ Daemon arrêté (s'il était actif)"

# Installation
install: build
	@echo "📦 Installation des binaires..."
	@mkdir -p $(INSTALL_DIR)
	@cp target/release/encodetalker-daemon $(INSTALL_DIR)/
	@cp target/release/encodetalker-tui $(INSTALL_DIR)/
	@chmod +x $(INSTALL_DIR)/encodetalker-daemon
	@chmod +x $(INSTALL_DIR)/encodetalker-tui
	@echo "✅ Binaires installés dans $(INSTALL_DIR)/"
	@echo ""
	@echo "Vous pouvez maintenant lancer:"
	@echo "  encodetalker-tui"

uninstall:
	@echo "🗑️  Désinstallation des binaires..."
	@rm -f $(INSTALL_DIR)/encodetalker-daemon
	@rm -f $(INSTALL_DIR)/encodetalker-tui
	@echo "✅ Binaires désinstallés"

# Informations système
info:
	@echo "📊 Informations système:"
	@echo ""
	@echo "Répertoires:"
	@echo "  Data:   $(DATA_DIR)"
	@echo "  Config: $(CONFIG_DIR)"
	@echo "  Install: $(INSTALL_DIR)"
	@echo ""
	@echo "Dépendances compilées:"
	@if [ -d "$(DATA_DIR)/deps/bin" ]; then \
		ls -lh $(DATA_DIR)/deps/bin/ 2>/dev/null || echo "  Aucune"; \
	else \
		echo "  Aucune"; \
	fi
	@echo ""
	@echo "Processus daemon:"
	@pgrep -f -l encodetalker-daemon || echo "  Non actif"
	@echo ""
	@echo "Socket:"
	@if [ -S "$(DATA_DIR)/daemon.sock" ]; then \
		ls -lh $(DATA_DIR)/daemon.sock; \
	else \
		echo "  Absent"; \
	fi

# Développement
dev-daemon:
	@echo "🔧 Lancement du daemon en mode développement..."
	RUST_LOG=debug $(CARGO) run --bin encodetalker-daemon

dev-tui:
	@echo "🔧 Lancement du TUI en mode développement..."
	RUST_LOG=debug $(CARGO) run --bin encodetalker-tui

# Watch mode (nécessite cargo-watch)
watch:
	@echo "👀 Watch mode (recompilation automatique)..."
	@if ! command -v cargo-watch >/dev/null 2>&1; then \
		echo "❌ cargo-watch n'est pas installé"; \
		echo "   Installez-le avec: cargo install cargo-watch"; \
		exit 1; \
	fi
	cargo watch -x build

# Benchmark (si jamais vous ajoutez des benchmarks)
bench:
	@echo "⚡ Lancement des benchmarks..."
	$(CARGO) bench

# Documentation
doc:
	@echo "📚 Génération de la documentation..."
	$(CARGO) doc --no-deps --open

# Release (pour préparer une release)
release: check build
	@echo "🎉 Build release prêt"
	@echo "   Binaires dans: ./target/release/"
	@ls -lh target/release/encodetalker-{daemon,tui}
