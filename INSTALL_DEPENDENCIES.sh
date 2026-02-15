#!/bin/bash

set -e  # Exit on error

# Couleurs pour output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Chemins par défaut (XDG Base Directory)
DEPS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/encodetalker/deps"
DEPS_BIN="$DEPS_DIR/bin"
DEPS_SRC="$DEPS_DIR/src"

# Configuration depuis config.toml (optionnel)
CONFIG_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/encodetalker/config.toml"

# Lire deps_dir custom depuis config.toml si présent
if [[ -f "$CONFIG_FILE" ]]; then
    custom_deps_dir=$(grep '^deps_dir' "$CONFIG_FILE" 2>/dev/null | sed 's/.*=\s*"\(.*\)"/\1/' | head -1)
    if [[ -n "$custom_deps_dir" ]]; then
        custom_deps_dir="${custom_deps_dir/#\~/$HOME}"
        DEPS_DIR="$custom_deps_dir"
        DEPS_BIN="$DEPS_DIR/bin"
        DEPS_SRC="$DEPS_DIR/src"
    fi
fi

# URLs sources (même que dans downloader.rs)
FFMPEG_VERSION="6.1"
FFMPEG_URL="https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz"
FFMPEG_WINDOWS_URL="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
SVT_AV1_GIT="https://github.com/BlueSwordM/svt-av1-psy.git"
LIBAOM_GIT="https://aomedia.googlesource.com/aom"

# Détection OS
OS="$(uname -s)"
case "$OS" in
    Linux*)     PLATFORM="linux";;
    Darwin*)    PLATFORM="macos";;
    MINGW*|MSYS*|CYGWIN*) PLATFORM="windows";;
    *)          PLATFORM="unknown";;
esac

# Nombre de cores CPU pour paralléliser make
NCPUS=$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)

#######################################
# Vérification des dépendances système
#######################################
check_system_dependencies() {
    echo -e "${YELLOW}=== Checking System Dependencies ===${NC}"

    local missing=()

    if [[ "$PLATFORM" == "linux" ]]; then
        # Linux : gcc, g++, make, cmake, git, nasm
        command -v gcc >/dev/null 2>&1 || missing+=("gcc")
        command -v g++ >/dev/null 2>&1 || missing+=("g++")
        command -v make >/dev/null 2>&1 || missing+=("make")
        command -v cmake >/dev/null 2>&1 || missing+=("cmake")
        command -v git >/dev/null 2>&1 || missing+=("git")
        command -v nasm >/dev/null 2>&1 || missing+=("nasm")

        if [[ ${#missing[@]} -gt 0 ]]; then
            echo -e "${RED}✗ Missing system dependencies: ${missing[*]}${NC}"
            echo ""
            echo "Please install them first:"
            echo "  # Arch/Manjaro:"
            echo "  sudo pacman -S base-devel cmake git nasm"
            echo ""
            echo "  # Ubuntu/Debian:"
            echo "  sudo apt install build-essential cmake git nasm"
            exit 1
        fi
    elif [[ "$PLATFORM" == "windows" ]]; then
        # Windows : cmake, git (gcc pas nécessaire car binaires pré-compilés)
        command -v cmake >/dev/null 2>&1 || missing+=("cmake")
        command -v git >/dev/null 2>&1 || missing+=("git")

        if [[ ${#missing[@]} -gt 0 ]]; then
            echo -e "${RED}✗ Missing system dependencies: ${missing[*]}${NC}"
            echo ""
            echo "Please install them first:"
            echo "  choco install cmake git"
            exit 1
        fi
    fi

    echo -e "${GREEN}✓ All system dependencies present${NC}"
    echo ""
}

#######################################
# Helper : Télécharger tarball
#######################################
download_tarball() {
    local url="$1"
    local dest_dir="$2"
    local tarball_name="$(basename "$url")"
    local tarball_path="$DEPS_SRC/$tarball_name"

    echo "  Downloading $tarball_name..."

    # Télécharger avec curl ou wget
    if command -v curl >/dev/null 2>&1; then
        curl -L -o "$tarball_path" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -O "$tarball_path" "$url"
    else
        echo -e "${RED}✗ Neither curl nor wget found${NC}"
        exit 1
    fi

    echo "  Extracting $tarball_name..."
    mkdir -p "$dest_dir"
    tar -xf "$tarball_path" -C "$dest_dir" --strip-components=1

    # Nettoyer tarball après extraction
    rm "$tarball_path"
}

#######################################
# Helper : Cloner repo Git
#######################################
clone_git_repo() {
    local url="$1"
    local dest_dir="$2"
    local depth="${3:-1}"  # Shallow clone par défaut

    echo "  Cloning $(basename "$url" .git)..."

    if [[ -d "$dest_dir" ]]; then
        echo "  Directory already exists, pulling latest..."
        git -C "$dest_dir" pull
    else
        git clone --depth "$depth" "$url" "$dest_dir"
    fi
}

#######################################
# Compilation FFmpeg (Linux)
#######################################
install_ffmpeg_linux() {
    echo -e "${YELLOW}=== Installing FFmpeg (Linux - source compilation) ===${NC}"

    local ffmpeg_src="$DEPS_SRC/ffmpeg-${FFMPEG_VERSION}"

    # Vérifier si déjà installé
    if [[ -x "$DEPS_BIN/ffmpeg" ]] && [[ -x "$DEPS_BIN/ffprobe" ]]; then
        echo -e "${GREEN}✓ FFmpeg already installed${NC}"
        return 0
    fi

    # Télécharger sources
    download_tarball "$FFMPEG_URL" "$ffmpeg_src"

    # Compiler
    echo "  Configuring FFmpeg... (this may take a while)"
    cd "$ffmpeg_src"

    ./configure \
        --prefix="$DEPS_DIR" \
        --bindir="$DEPS_BIN" \
        --enable-gpl \
        --enable-libopus \
        --enable-libvpx \
        --disable-doc \
        --disable-htmlpages \
        --disable-manpages \
        --disable-podpages \
        --disable-txtpages

    echo "  Building FFmpeg with $NCPUS cores... (~30-60 minutes)"
    make -j"$NCPUS"

    echo "  Installing FFmpeg..."
    make install

    # Vérifier installation
    if [[ -x "$DEPS_BIN/ffmpeg" ]] && [[ -x "$DEPS_BIN/ffprobe" ]]; then
        echo -e "${GREEN}✓ FFmpeg compiled successfully${NC}"
        "$DEPS_BIN/ffmpeg" -version | head -1
    else
        echo -e "${RED}✗ FFmpeg compilation failed${NC}"
        exit 1
    fi
}

#######################################
# Téléchargement FFmpeg (Windows)
#######################################
install_ffmpeg_windows() {
    echo -e "${YELLOW}=== Installing FFmpeg (Windows - pre-built binaries) ===${NC}"

    local ffmpeg_zip="$DEPS_SRC/ffmpeg-windows.zip"
    local ffmpeg_extract="$DEPS_SRC/ffmpeg-windows"

    # Vérifier si déjà installé
    if [[ -x "$DEPS_BIN/ffmpeg.exe" ]] && [[ -x "$DEPS_BIN/ffprobe.exe" ]]; then
        echo -e "${GREEN}✓ FFmpeg already installed${NC}"
        return 0
    fi

    # Télécharger binaires pré-compilés
    echo "  Downloading pre-built FFmpeg for Windows..."
    curl -L -o "$ffmpeg_zip" "$FFMPEG_WINDOWS_URL"

    # Extraire
    echo "  Extracting FFmpeg..."
    mkdir -p "$ffmpeg_extract"
    unzip -q "$ffmpeg_zip" -d "$ffmpeg_extract"

    # Copier binaires (structure: ffmpeg-xxx/bin/ffmpeg.exe)
    local bin_dir=$(find "$ffmpeg_extract" -type d -name "bin" | head -1)
    cp "$bin_dir/ffmpeg.exe" "$DEPS_BIN/"
    cp "$bin_dir/ffprobe.exe" "$DEPS_BIN/"

    # Nettoyer
    rm "$ffmpeg_zip"
    rm -rf "$ffmpeg_extract"

    # Vérifier installation
    if [[ -x "$DEPS_BIN/ffmpeg.exe" ]]; then
        echo -e "${GREEN}✓ FFmpeg installed successfully${NC}"
        "$DEPS_BIN/ffmpeg.exe" -version | head -1
    else
        echo -e "${RED}✗ FFmpeg installation failed${NC}"
        exit 1
    fi
}

#######################################
# Compilation SVT-AV1-PSY
#######################################
install_svt_av1() {
    echo -e "${YELLOW}=== Installing SVT-AV1-PSY ===${NC}"

    local svt_src="$DEPS_SRC/svt-av1-psy"
    local svt_build="$svt_src/Build"

    # Vérifier si déjà installé
    if [[ -x "$DEPS_BIN/SvtAv1EncApp" ]]; then
        echo -e "${GREEN}✓ SVT-AV1-PSY already installed${NC}"
        return 0
    fi

    # Cloner repo Git
    clone_git_repo "$SVT_AV1_GIT" "$svt_src" 1

    # Créer dossier build
    mkdir -p "$svt_build"
    cd "$svt_build"

    # Configurer avec CMake
    echo "  Configuring SVT-AV1-PSY..."
    cmake .. \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX="$DEPS_DIR" \
        -DBUILD_SHARED_LIBS=OFF

    # Compiler
    echo "  Building SVT-AV1-PSY with $NCPUS cores... (~15-30 minutes)"
    make -j"$NCPUS"

    # Installer
    echo "  Installing SVT-AV1-PSY..."
    make install

    # Copier binaire si pas dans bin/ (parfois dans local/bin/)
    if [[ ! -x "$DEPS_BIN/SvtAv1EncApp" ]]; then
        find "$DEPS_DIR" -name "SvtAv1EncApp" -type f -exec cp {} "$DEPS_BIN/" \;
    fi

    # Vérifier installation
    if [[ -x "$DEPS_BIN/SvtAv1EncApp" ]]; then
        echo -e "${GREEN}✓ SVT-AV1-PSY compiled successfully${NC}"
        "$DEPS_BIN/SvtAv1EncApp" --version 2>&1 | head -1 || true
    else
        echo -e "${RED}✗ SVT-AV1-PSY compilation failed${NC}"
        exit 1
    fi
}

#######################################
# Compilation libaom
#######################################
install_aomenc() {
    echo -e "${YELLOW}=== Installing libaom (aomenc) ===${NC}"

    local aom_src="$DEPS_SRC/aom"
    local aom_build="$aom_src/build"

    # Vérifier si déjà installé
    if [[ -x "$DEPS_BIN/aomenc" ]]; then
        echo -e "${GREEN}✓ libaom already installed${NC}"
        return 0
    fi

    # Cloner repo Git
    clone_git_repo "$LIBAOM_GIT" "$aom_src" 1

    # Créer dossier build
    mkdir -p "$aom_build"
    cd "$aom_build"

    # Configurer avec CMake
    echo "  Configuring libaom..."
    cmake .. \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX="$DEPS_DIR" \
        -DENABLE_DOCS=0 \
        -DENABLE_TESTS=0 \
        -DENABLE_EXAMPLES=1

    # Compiler
    echo "  Building libaom with $NCPUS cores... (~15-30 minutes)"
    make -j"$NCPUS"

    # Installer
    echo "  Installing libaom..."
    make install

    # Copier binaire si pas dans bin/
    if [[ ! -x "$DEPS_BIN/aomenc" ]]; then
        find "$DEPS_DIR" -name "aomenc" -type f -exec cp {} "$DEPS_BIN/" \;
    fi

    # Vérifier installation
    if [[ -x "$DEPS_BIN/aomenc" ]]; then
        echo -e "${GREEN}✓ libaom compiled successfully${NC}"
        "$DEPS_BIN/aomenc" --help 2>&1 | head -1 || true
    else
        echo -e "${RED}✗ libaom compilation failed${NC}"
        exit 1
    fi
}

#######################################
# Usage
#######################################
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Install/compile EncodeTalker dependencies"
    echo ""
    echo "OPTIONS:"
    echo "  --all             Install all dependencies (default)"
    echo "  --ffmpeg          Install only FFmpeg"
    echo "  --svt-av1         Install only SVT-AV1-PSY"
    echo "  --aomenc          Install only libaom (aomenc)"
    echo "  --skip-check      Skip system dependencies check"
    echo "  -h, --help        Show this help"
    echo ""
    echo "EXAMPLES:"
    echo "  $0                    # Install all dependencies"
    echo "  $0 --ffmpeg          # Install only FFmpeg"
    echo "  $0 --svt-av1 --aomenc # Install SVT-AV1 and libaom"
}

#######################################
# Main
#######################################
main() {
    local install_all=true
    local install_ffmpeg=false
    local install_svt=false
    local install_aom=false
    local skip_check=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --all)
                install_all=true
                shift
                ;;
            --ffmpeg)
                install_all=false
                install_ffmpeg=true
                shift
                ;;
            --svt-av1)
                install_all=false
                install_svt=true
                shift
                ;;
            --aomenc)
                install_all=false
                install_aom=true
                shift
                ;;
            --skip-check)
                skip_check=true
                shift
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                echo "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
    done

    # Si --all ou aucun flag, installer tout
    if [[ "$install_all" == true ]]; then
        install_ffmpeg=true
        install_svt=true
        install_aom=true
    fi

    echo -e "${GREEN}=== EncodeTalker Dependencies Installation ===${NC}"
    echo ""
    echo "Configuration:"
    echo "  Platform:  $PLATFORM"
    echo "  Deps dir:  $DEPS_DIR"
    echo "  Deps bin:  $DEPS_BIN"
    echo "  Deps src:  $DEPS_SRC"
    echo "  CPU cores: $NCPUS"
    echo ""

    # Créer dossiers
    mkdir -p "$DEPS_BIN" "$DEPS_SRC"

    # Vérifier dépendances système
    if [[ "$skip_check" == false ]]; then
        check_system_dependencies
    fi

    # Installer les dépendances demandées
    local start_time=$(date +%s)

    if [[ "$install_ffmpeg" == true ]]; then
        if [[ "$PLATFORM" == "linux" ]]; then
            install_ffmpeg_linux
        elif [[ "$PLATFORM" == "windows" ]]; then
            install_ffmpeg_windows
        else
            echo -e "${RED}✗ FFmpeg installation not supported on $PLATFORM${NC}"
        fi
        echo ""
    fi

    if [[ "$install_svt" == true ]]; then
        if [[ "$PLATFORM" == "linux" ]]; then
            install_svt_av1
        else
            echo -e "${YELLOW}⚠ SVT-AV1-PSY compilation not yet supported on $PLATFORM${NC}"
        fi
        echo ""
    fi

    if [[ "$install_aom" == true ]]; then
        if [[ "$PLATFORM" == "linux" ]]; then
            install_aomenc
        else
            echo -e "${YELLOW}⚠ libaom compilation not yet supported on $PLATFORM${NC}"
        fi
        echo ""
    fi

    local end_time=$(date +%s)
    local elapsed=$((end_time - start_time))
    local minutes=$((elapsed / 60))
    local seconds=$((elapsed % 60))

    echo -e "${GREEN}=== Installation Complete ===${NC}"
    echo "Total time: ${minutes}m ${seconds}s"
    echo ""
    echo "Verify installation with:"
    echo "  ./CHECK_INSTALLED_DEPENDENCIES.sh"
    echo ""
    echo "You can now run:"
    echo "  ./target/release/encodetalker-tui"
}

main "$@"
