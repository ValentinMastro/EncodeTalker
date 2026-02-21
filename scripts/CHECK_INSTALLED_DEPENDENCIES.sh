#!/bin/bash

# INSTALL.sh - Script de vérification des dépendances EncodeTalker
# Vérifie que FFmpeg, SVT-AV1-PSY et libaom sont installés et fonctionnels

set -e

# Déterminer le répertoire de l'exécutable (MODE PORTABLE)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXE_DIR="$(dirname "$SCRIPT_DIR")"  # Parent de scripts/

# Chemins par défaut (MODE PORTABLE)
DEPS_DIR="$EXE_DIR/.dependencies"
DEPS_BIN="$DEPS_DIR/bin"

# Compteurs
DEPS_OK=0
DEPS_MISSING=0

# Lire deps_dir custom depuis config.toml si présent
CONFIG_FILE="$EXE_DIR/config.toml"
if [[ -f "$CONFIG_FILE" ]]; then
    custom_deps_dir=$(grep '^\s*deps_dir\s*=' "$CONFIG_FILE" 2>/dev/null | sed 's/.*=\s*"\(.*\)"/\1/' | head -1)
    if [[ -n "$custom_deps_dir" ]]; then
        # Expand tilde et variables
        custom_deps_dir="${custom_deps_dir/#\~/$HOME}"
        custom_deps_dir=$(eval echo "$custom_deps_dir")
        DEPS_DIR="$custom_deps_dir"
        DEPS_BIN="$DEPS_DIR/bin"
    fi
fi

# Fonction 0 : Vérifier libopus
check_opus() {
    echo "Checking libopus..."

    local status="✗"

    if [[ -f "$DEPS_DIR/lib/libopus.a" ]] || [[ -f "$DEPS_DIR/lib/libopus.so" ]]; then
        status="✓"
        DEPS_OK=$((DEPS_OK + 1))
        echo "  $status libopus: $DEPS_DIR/lib/"
        return 0
    fi

    DEPS_MISSING=$((DEPS_MISSING + 1))
    echo "  $status libopus: MISSING"
    echo "     Expected at: $DEPS_DIR/lib/libopus.a"
}

# Fonction 1 : Vérifier libvpx
check_libvpx() {
    echo "Checking libvpx..."

    local status="✗"

    if [[ -f "$DEPS_DIR/lib/libvpx.a" ]] || [[ -f "$DEPS_DIR/lib/libvpx.so" ]]; then
        status="✓"
        DEPS_OK=$((DEPS_OK + 1))
        echo "  $status libvpx: $DEPS_DIR/lib/"
        return 0
    fi

    DEPS_MISSING=$((DEPS_MISSING + 1))
    echo "  $status libvpx: MISSING"
    echo "     Expected at: $DEPS_DIR/lib/libvpx.a"
}

# Fonction 2 : Vérifier FFmpeg + FFprobe
check_ffmpeg() {
    echo "Checking FFmpeg..."

    local ffmpeg_path="$DEPS_BIN/ffmpeg"
    local ffprobe_path="$DEPS_BIN/ffprobe"
    local status="✗"

    if [[ -x "$ffmpeg_path" ]] && [[ -x "$ffprobe_path" ]]; then
        # Tester que les binaires fonctionnent
        if "$ffmpeg_path" -version &>/dev/null && "$ffprobe_path" -version &>/dev/null; then
            status="✓"
            DEPS_OK=$((DEPS_OK + 1))
            echo "  $status FFmpeg:  $ffmpeg_path"
            echo "  $status FFprobe: $ffprobe_path"
            return 0
        fi
    fi

    DEPS_MISSING=$((DEPS_MISSING + 1))
    echo "  $status FFmpeg: MISSING or not executable"
    echo "     Expected at: $ffmpeg_path"
    echo "     Expected at: $ffprobe_path"
}

# Fonction 2 : Vérifier SVT-AV1-PSY
check_svt_av1() {
    echo "Checking SVT-AV1-PSY..."

    local binary_path="$DEPS_BIN/SvtAv1EncApp"
    local status="✗"

    if [[ -x "$binary_path" ]]; then
        # Note: SvtAv1EncApp peut retourner exit code != 0 avec --version
        # Vérifier que la commande produit une sortie contenant "SVT"
        if "$binary_path" --version 2>&1 | grep -q "SVT"; then
            status="✓"
            DEPS_OK=$((DEPS_OK + 1))
            echo "  $status SvtAv1EncApp: $binary_path"
            return 0
        fi
    fi

    DEPS_MISSING=$((DEPS_MISSING + 1))
    echo "  $status SvtAv1EncApp: MISSING or not executable"
    echo "     Expected at: $binary_path"
}

# Fonction 3 : Vérifier libaom (aomenc)
check_aomenc() {
    echo "Checking libaom (aomenc)..."

    local binary_path="$DEPS_BIN/aomenc"
    local status="✗"

    if [[ -x "$binary_path" ]]; then
        # aomenc peut ne pas supporter --version, utiliser --help
        if "$binary_path" --help 2>&1 | grep -q "AV1"; then
            status="✓"
            DEPS_OK=$((DEPS_OK + 1))
            echo "  $status aomenc: $binary_path"
            return 0
        fi
    fi

    DEPS_MISSING=$((DEPS_MISSING + 1))
    echo "  $status aomenc: MISSING or not executable"
    echo "     Expected at: $binary_path"
}

# Fonction principale
main() {
    echo "=== EncodeTalker Dependencies Verification ==="
    echo ""
    echo "Configuration:"
    echo "  Data dir:  $(dirname "$DEPS_DIR")"
    echo "  Deps dir:  $DEPS_DIR"
    echo "  Deps bin:  $DEPS_BIN"
    echo ""

    # Si config.toml existe, mentionner qu'il peut override les chemins
    if [[ -f "$CONFIG_FILE" ]]; then
        echo "  Config:    $CONFIG_FILE (custom paths may override defaults)"
        echo ""
    fi

    echo "Compiled Dependencies:"
    check_opus
    check_libvpx
    check_ffmpeg
    check_svt_av1
    check_aomenc

    echo ""
    echo "=== Summary ==="
    echo "  Dependencies OK:      $DEPS_OK/5"
    echo "  Dependencies missing: $DEPS_MISSING/5"

    if [[ $DEPS_MISSING -eq 0 ]]; then
        exit 0
    else
        exit 1
    fi
}

main "$@"
