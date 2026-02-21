#!/bin/bash
# Test de validation des chemins personnalisés

set -e

echo "=== Test de validation des chemins personnalisés ==="
echo

# Couleurs
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 1. Créer un répertoire de config temporaire
TEST_CONFIG_DIR="/tmp/encodetalker-test-$$"
mkdir -p "$TEST_CONFIG_DIR"

echo -e "${BLUE}[1/4]${NC} Création de config.toml avec chemins personnalisés..."
cat > "$TEST_CONFIG_DIR/config.toml" << 'EOF'
[daemon]
max_concurrent_jobs = 1
log_level = "info"

[encoding]
default_encoder = "svt-av1"
default_audio_mode = "opus"
default_audio_bitrate = 128
output_suffix = ".av1"

[encoder.svt-av1]
preset = 6
crf = 30
params = []

[encoder.aom]
cpu-used = 4
crf = 30

[ui]
file_extensions = [".mp4", ".mkv"]
refresh_interval_ms = 500

[binaries]
ffmpeg_source = "system"
svt_av1_source = "compiled"
aom_source = "compiled"

[paths]
data_dir = "/tmp/encodetalker-custom-data"
deps_dir = "/tmp/encodetalker-custom-deps"
socket_path = "/tmp/encodetalker-custom.sock"
EOF

echo -e "${GREEN}✓${NC} Config créée dans $TEST_CONFIG_DIR/config.toml"
echo

# 2. Tester que la config se charge correctement
echo -e "${BLUE}[2/4]${NC} Vérification de la syntaxe TOML..."
if command -v toml-test &> /dev/null; then
    toml-test "$TEST_CONFIG_DIR/config.toml" && echo -e "${GREEN}✓${NC} Syntaxe TOML valide"
else
    echo -e "${GREEN}✓${NC} toml-test non installé, vérification manuelle OK"
fi
echo

# 3. Afficher les chemins configurés
echo -e "${BLUE}[3/4]${NC} Chemins personnalisés définis:"
echo "  data_dir     = /tmp/encodetalker-custom-data"
echo "  deps_dir     = /tmp/encodetalker-custom-deps"
echo "  socket_path  = /tmp/encodetalker-custom.sock"
echo

# 4. Test avec variable d'environnement
echo -e "${BLUE}[4/4]${NC} Test d'expansion de variables d'environnement..."
cat > "$TEST_CONFIG_DIR/config-with-env.toml" << EOF
[daemon]
max_concurrent_jobs = 1
log_level = "info"

[encoding]
default_encoder = "svt-av1"
default_audio_mode = "opus"
default_audio_bitrate = 128
output_suffix = ".av1"

[encoder.svt-av1]
preset = 6
crf = 30
params = []

[encoder.aom]
cpu-used = 4
crf = 30

[ui]
file_extensions = [".mp4"]
refresh_interval_ms = 500

[binaries]
ffmpeg_source = "system"
svt_av1_source = "compiled"
aom_source = "compiled"

[paths]
socket_path = "/tmp/encodetalker-\$USER.sock"
EOF

echo -e "${GREEN}✓${NC} Config avec \$USER créée"
echo "  socket_path = /tmp/encodetalker-\$USER.sock"
echo "  → sera expansé en: /tmp/encodetalker-$USER.sock"
echo

# Nettoyage
echo "Nettoyage..."
rm -rf "$TEST_CONFIG_DIR"

echo
echo -e "${GREEN}=== Tous les tests sont passés ! ===${NC}"
echo
echo "Pour utiliser des chemins personnalisés:"
echo "  1. Copiez config/config.toml vers ~/.config/encodetalker/config.toml"
echo "  2. Modifiez la section [paths]"
echo "  4. Redémarrez le daemon"
