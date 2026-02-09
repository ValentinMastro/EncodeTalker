# Makefile pour EncodeTalker
# Facilite la compilation, le nettoyage et le lancement du projet

.PHONY: all build build-dev test clean clean-all fmt clippy run-daemon run-tui install help

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
	@echo "  make test        - Lancer les tests"
	@echo "  make clean       - Nettoyer le build Cargo + dépendances compilées"
	@echo "  make clean-all   - Nettoyer tout (build + dépendances + socket)"
	@echo "  make fmt         - Formatter le code"
	@echo "  make clippy      - Linter avec clippy"
	@echo "  make check       - Vérifier (fmt + clippy + test)"
	@echo "  make run-daemon  - Lancer le daemon avec logs"
	@echo "  make run-tui     - Lancer le TUI"
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

# Tests
test:
	@echo "🧪 Lancement des tests..."
	$(CARGO) test --all

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
	@echo "🧹 Suppression des fichiers .log..."
	@find . -name "*.log" -type f -delete 2>/dev/null || true
	@echo "🧹 Arrêt du daemon si en cours..."
	@pkill encodetalker-daemon 2>/dev/null || true
	@echo "✅ Nettoyage complet terminé"

# Lancement
run-daemon:
	@echo "🚀 Lancement du daemon..."
	@if pgrep -x encodetalker-daemon > /dev/null; then \
		echo "⚠️  Le daemon est déjà en cours d'exécution"; \
		echo "   Arrêtez-le avec: pkill encodetalker-daemon"; \
		exit 1; \
	fi
	RUST_LOG=info ./target/release/encodetalker-daemon

run-tui:
	@echo "🖥️  Lancement du TUI..."
	./target/release/encodetalker-tui

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
	@pgrep -l encodetalker-daemon || echo "  Non actif"
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
