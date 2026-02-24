#!/bin/bash

set -e  # Exit on error

# Couleurs pour output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Déterminer le répertoire de l'exécutable (MODE PORTABLE)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXE_DIR="$(dirname "$SCRIPT_DIR")"  # Parent de scripts/

# Chemins par défaut (MODE PORTABLE)
DEPS_DIR="$EXE_DIR/.dependencies"
DEPS_BIN="$DEPS_DIR/bin"
DEPS_SRC="$DEPS_DIR/src"

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
        DEPS_SRC="$DEPS_DIR/src"
    fi
fi

# URLs sources (même que dans downloader.rs)
OPUS_VERSION="1.6.1"
OPUS_URL="https://downloads.xiph.org/releases/opus/opus-${OPUS_VERSION}.tar.gz"
LIBVPX_VERSION="1.16.0"
LIBVPX_URL="https://github.com/webmproject/libvpx/archive/refs/tags/v${LIBVPX_VERSION}.tar.gz"
FFMPEG_VERSION="8.0.1"
FFMPEG_URL="https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz"
FFMPEG_WINDOWS_URL="https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
SVT_AV1_GIT="https://github.com/BlueSwordM/svt-av1-psy.git"
LIBAOM_GIT="https://aomedia.googlesource.com/aom"
VMAF_GIT="https://github.com/Netflix/vmaf.git"
DAV1D_VERSION="1.5.3"
DAV1D_URL="https://code.videolan.org/videolan/dav1d/-/archive/${DAV1D_VERSION}/dav1d-${DAV1D_VERSION}.tar.gz"
MESON_VERSION="1.10.1"
MESON_URL="https://github.com/mesonbuild/meson/releases/download/${MESON_VERSION}/meson-${MESON_VERSION}.tar.gz"
NINJA_VERSION="1.13.2"
NINJA_URL="https://github.com/ninja-build/ninja/releases/download/v${NINJA_VERSION}/ninja-linux.zip"
CMAKE_VERSION="3.31.5"
CMAKE_URL="https://github.com/Kitware/CMake/releases/download/v${CMAKE_VERSION}/cmake-${CMAKE_VERSION}-linux-x86_64.tar.gz"

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
# Installer CMake localement
#######################################
ensure_cmake() {
    local cmake_dir="$DEPS_DIR/cmake-${CMAKE_VERSION}-linux-x86_64"
    local cmake_bin="$cmake_dir/bin"

    # Vérifier si déjà téléchargé
    if [[ -x "$cmake_bin/cmake" ]]; then
        echo -e "${GREEN}✓ Local CMake already available: $("$cmake_bin/cmake" --version | head -1)${NC}"
        export PATH="$cmake_bin:$PATH"
        return 0
    fi

    echo -e "${YELLOW}=== Installing CMake ${CMAKE_VERSION} ===${NC}"
    local cmake_tarball="$DEPS_SRC/cmake-${CMAKE_VERSION}-linux-x86_64.tar.gz"

    if command -v curl >/dev/null 2>&1; then
        curl -L -o "$cmake_tarball" "$CMAKE_URL"
    elif command -v wget >/dev/null 2>&1; then
        wget -O "$cmake_tarball" "$CMAKE_URL"
    else
        echo -e "${RED}✗ Neither curl nor wget found, cannot download CMake${NC}"
        exit 1
    fi

    echo "  Extracting CMake..."
    tar -xzf "$cmake_tarball" -C "$DEPS_DIR"
    rm "$cmake_tarball"

    # Ajouter au PATH
    export PATH="$cmake_bin:$PATH"

    if cmake --version >/dev/null 2>&1; then
        echo -e "${GREEN}✓ CMake ${CMAKE_VERSION} installed${NC}"
    else
        echo -e "${RED}✗ Failed to install CMake locally${NC}"
        exit 1
    fi
}

#######################################
# Installer Meson et Ninja localement
#######################################
ensure_meson_ninja() {
    # --- Ninja ---
    if [[ -x "$DEPS_BIN/ninja" ]]; then
        echo -e "${GREEN}✓ Local Ninja already available: $("$DEPS_BIN/ninja" --version)${NC}"
    else
        echo -e "${YELLOW}=== Installing Ninja ${NINJA_VERSION} ===${NC}"
        local ninja_zip="$DEPS_SRC/ninja-linux.zip"

        if command -v curl >/dev/null 2>&1; then
            curl -L -o "$ninja_zip" "$NINJA_URL"
        elif command -v wget >/dev/null 2>&1; then
            wget -O "$ninja_zip" "$NINJA_URL"
        else
            echo -e "${RED}✗ Neither curl nor wget found, cannot download Ninja${NC}"
            exit 1
        fi

        echo "  Extracting Ninja..."
        python3 -c "import zipfile; zipfile.ZipFile('$ninja_zip').extractall('$DEPS_BIN')"
        chmod +x "$DEPS_BIN/ninja"
        rm "$ninja_zip"

        if [[ -x "$DEPS_BIN/ninja" ]]; then
            echo -e "${GREEN}✓ Ninja ${NINJA_VERSION} installed${NC}"
        else
            echo -e "${RED}✗ Failed to install Ninja locally${NC}"
            exit 1
        fi
    fi

    # --- Meson ---
    local meson_dir="$DEPS_DIR/meson-${MESON_VERSION}"

    if [[ -x "$DEPS_BIN/meson" ]]; then
        echo -e "${GREEN}✓ Local Meson already available${NC}"
    else
        echo -e "${YELLOW}=== Installing Meson ${MESON_VERSION} ===${NC}"
        local meson_tarball="$DEPS_SRC/meson-${MESON_VERSION}.tar.gz"

        if command -v curl >/dev/null 2>&1; then
            curl -L -o "$meson_tarball" "$MESON_URL"
        elif command -v wget >/dev/null 2>&1; then
            wget -O "$meson_tarball" "$MESON_URL"
        else
            echo -e "${RED}✗ Neither curl nor wget found, cannot download Meson${NC}"
            exit 1
        fi

        echo "  Extracting Meson..."
        mkdir -p "$meson_dir"
        tar -xzf "$meson_tarball" -C "$DEPS_DIR"
        rm "$meson_tarball"

        # Créer un wrapper script pour meson
        cat > "$DEPS_BIN/meson" << EOF
#!/bin/sh
exec python3 "$meson_dir/meson.py" "\$@"
EOF
        chmod +x "$DEPS_BIN/meson"

        if [[ -x "$DEPS_BIN/meson" ]]; then
            echo -e "${GREEN}✓ Meson ${MESON_VERSION} installed${NC}"
        else
            echo -e "${RED}✗ Failed to install Meson locally${NC}"
            exit 1
        fi
    fi

    # S'assurer que $DEPS_BIN est dans le PATH
    export PATH="$DEPS_BIN:$PATH"
}

#######################################
# Vérification des dépendances système
#######################################
check_system_dependencies() {
    echo -e "${YELLOW}=== Checking System Dependencies ===${NC}"

    local missing=()

    if [[ "$PLATFORM" == "linux" ]]; then
        # Linux : gcc, g++, make, git, nasm (cmake géré par ensure_cmake)
        command -v gcc >/dev/null 2>&1 || missing+=("gcc")
        command -v g++ >/dev/null 2>&1 || missing+=("g++")
        command -v make >/dev/null 2>&1 || missing+=("make")
        command -v git >/dev/null 2>&1 || missing+=("git")
        command -v nasm >/dev/null 2>&1 || missing+=("nasm")
        command -v python3 >/dev/null 2>&1 || missing+=("python3")

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
        # Windows : git (gcc/cmake pas nécessaires car binaires pré-compilés)
        command -v git >/dev/null 2>&1 || missing+=("git")

        if [[ ${#missing[@]} -gt 0 ]]; then
            echo -e "${RED}✗ Missing system dependencies: ${missing[*]}${NC}"
            echo ""
            echo "Please install them first:"
            echo "  choco install git"
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
# Compilation libopus (Linux)
#######################################
install_opus() {
    echo -e "${YELLOW}=== Installing libopus ===${NC}"

    local opus_src="$DEPS_SRC/opus-${OPUS_VERSION}"

    # Vérifier si déjà installé
    if [[ -f "$DEPS_DIR/lib/libopus.a" ]] || [[ -f "$DEPS_DIR/lib/libopus.so" ]]; then
        echo -e "${GREEN}✓ libopus already installed${NC}"
        return 0
    fi

    # Télécharger sources
    download_tarball "$OPUS_URL" "$opus_src"

    # Compiler
    echo "  Configuring libopus..."
    cd "$opus_src"

    ./configure \
        --prefix="$DEPS_DIR" \
        --disable-doc \
        --disable-extra-programs

    echo "  Building libopus with $NCPUS cores..."
    make -j"$NCPUS"

    echo "  Installing libopus..."
    make install

    # Vérifier installation
    if [[ -f "$DEPS_DIR/lib/libopus.a" ]] || [[ -f "$DEPS_DIR/lib/libopus.so" ]]; then
        echo -e "${GREEN}✓ libopus compiled successfully${NC}"
    else
        echo -e "${RED}✗ libopus compilation failed${NC}"
        exit 1
    fi
}

#######################################
# Compilation libvpx (Linux)
#######################################
install_libvpx() {
    echo -e "${YELLOW}=== Installing libvpx ===${NC}"

    local vpx_src="$DEPS_SRC/libvpx-${LIBVPX_VERSION}"

    # Vérifier si déjà installé
    if [[ -f "$DEPS_DIR/lib/libvpx.a" ]] || [[ -f "$DEPS_DIR/lib/libvpx.so" ]]; then
        echo -e "${GREEN}✓ libvpx already installed${NC}"
        return 0
    fi

    # Télécharger sources
    download_tarball "$LIBVPX_URL" "$vpx_src"

    # Compiler
    echo "  Configuring libvpx..."
    cd "$vpx_src"

    ./configure \
        --prefix="$DEPS_DIR" \
        --disable-examples \
        --disable-unit-tests \
        --disable-docs \
        --enable-vp9-highbitdepth

    echo "  Building libvpx with $NCPUS cores..."
    make -j"$NCPUS"

    echo "  Installing libvpx..."
    make install

    # Vérifier installation
    if [[ -f "$DEPS_DIR/lib/libvpx.a" ]] || [[ -f "$DEPS_DIR/lib/libvpx.so" ]]; then
        echo -e "${GREEN}✓ libvpx compiled successfully${NC}"
    else
        echo -e "${RED}✗ libvpx compilation failed${NC}"
        exit 1
    fi
}

#######################################
# Compilation libdav1d (Linux)
#######################################
install_dav1d() {
    echo -e "${YELLOW}=== Installing libdav1d ===${NC}"

    local dav1d_src="$DEPS_SRC/dav1d-${DAV1D_VERSION}"

    # Vérifier si déjà installé
    if [[ -f "$DEPS_DIR/lib/libdav1d.a" ]] || [[ -f "$DEPS_DIR/lib/libdav1d.so" ]]; then
        echo -e "${GREEN}✓ libdav1d already installed${NC}"
        return 0
    fi

    # Télécharger sources
    download_tarball "$DAV1D_URL" "$dav1d_src"

    # Compiler
    echo "  Configuring libdav1d..."
    cd "$dav1d_src"

    meson setup build \
        --prefix="$DEPS_DIR" \
        --libdir=lib \
        --default-library=static \
        --buildtype=release \
        -Denable_float=true \
        -Denable_tools=false \
        -Denable_tests=false

    echo "  Building libdav1d with $NCPUS cores..."
    ninja -C build -j"$NCPUS"

    echo "  Installing libdav1d..."
    ninja -C build install

    # Vérifier installation (peut être dans lib/ ou lib/x86_64-linux-gnu/)
    if [[ -f "$DEPS_DIR/lib/libdav1d.a" ]] || [[ -f "$DEPS_DIR/lib/libdav1d.so" ]] || \
       find "$DEPS_DIR/lib" -name "libdav1d.*" -print -quit 2>/dev/null | grep -q .; then
        echo -e "${GREEN}✓ libdav1d compiled successfully${NC}"
    else
        echo -e "${RED}✗ libdav1d compilation failed${NC}"
        exit 1
    fi
}

#######################################
# Compilation libvmaf (Linux)
#######################################
install_vmaf() {
    echo -e "${YELLOW}=== Installing libvmaf ===${NC}"

    local vmaf_src="$DEPS_SRC/vmaf"

    # Vérifier si déjà installé
    if [[ -f "$DEPS_DIR/lib/libvmaf.a" ]] || [[ -f "$DEPS_DIR/lib/libvmaf.so" ]]; then
        echo -e "${GREEN}✓ libvmaf already installed${NC}"
        return 0
    fi

    # Cloner repo Git
    clone_git_repo "$VMAF_GIT" "$vmaf_src" 1

    # Compiler avec Meson/Ninja (dans le sous-répertoire libvmaf)
    echo "  Configuring libvmaf..."
    cd "$vmaf_src/libvmaf"

    # Nettoyer un éventuel build précédent échoué
    if [[ -d "build" ]]; then
        rm -rf "build"
    fi

    meson setup build \
        --prefix="$DEPS_DIR" \
        --libdir=lib \
        --default-library=static \
        --buildtype=release \
        -Denable_float=true

    echo "  Building libvmaf with $NCPUS cores..."
    ninja -C build -j"$NCPUS"

    echo "  Installing libvmaf..."
    ninja -C build install

    # Vérifier installation
    if [[ -f "$DEPS_DIR/lib/libvmaf.a" ]] || [[ -f "$DEPS_DIR/lib/libvmaf.so" ]] || \
       find "$DEPS_DIR/lib" -name "libvmaf.*" -print -quit 2>/dev/null | grep -q .; then
        echo -e "${GREEN}✓ libvmaf compiled successfully${NC}"
    else
        echo -e "${RED}✗ libvmaf compilation failed${NC}"
        exit 1
    fi
}

#######################################
# Compilation FFmpeg (Linux)
#######################################
install_ffmpeg_linux() {
    echo -e "${YELLOW}=== Installing FFmpeg (Linux - source compilation) ===${NC}"

    local ffmpeg_src="$DEPS_SRC/ffmpeg-${FFMPEG_VERSION}"

    # Vérifier si déjà installé (avec toutes les libs nécessaires)
    if [[ -x "$DEPS_BIN/ffmpeg" ]] && [[ -x "$DEPS_BIN/ffprobe" ]]; then
        # Vérifier que FFmpeg est compilé avec libvmaf
        if "$DEPS_BIN/ffmpeg" -filters 2>/dev/null | grep -q "libvmaf"; then
            echo -e "${GREEN}✓ FFmpeg already installed (with libvmaf)${NC}"
            return 0
        else
            echo -e "${YELLOW}⚠ FFmpeg installed but missing libvmaf, recompiling...${NC}"
            rm -f "$DEPS_BIN/ffmpeg" "$DEPS_BIN/ffprobe"
        fi
    fi

    # Télécharger sources (si pas déjà présentes)
    if [[ ! -d "$ffmpeg_src" ]]; then
        download_tarball "$FFMPEG_URL" "$ffmpeg_src"
    fi

    # Compiler
    echo "  Configuring FFmpeg... (this may take a while)"
    cd "$ffmpeg_src"

    # Nettoyer un éventuel build précédent (important si on recompile avec de nouvelles libs)
    if [[ -f "ffbuild/config.mak" ]]; then
        echo "  Cleaning previous FFmpeg build..."
        make distclean 2>/dev/null || true
    fi

    # Rendre les libs compilées localement visibles par pkg-config et le linker
    export PKG_CONFIG_PATH="$DEPS_DIR/lib/pkgconfig:${PKG_CONFIG_PATH:-}"

    # Ajouter -lstdc++ pour linker libvmaf qui contient du C++
    ./configure \
        --prefix="$DEPS_DIR" \
        --bindir="$DEPS_BIN" \
        --extra-cflags="-I$DEPS_DIR/include" \
        --extra-ldflags="-L$DEPS_DIR/lib" \
        --extra-libs="-lstdc++ -lm -lpthread" \
        --enable-gpl \
        --enable-libopus \
        --enable-libvpx \
        --enable-libdav1d \
        --enable-libvmaf \
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
    echo "  --opus            Install only libopus"
    echo "  --vpx             Install only libvpx"
    echo "  --ffmpeg          Install only FFmpeg (compile libopus/libvpx first si absents)"
    echo "  --svt-av1         Install only SVT-AV1-PSY"
    echo "  --dav1d            Install only libdav1d"
    echo "  --vmaf            Install only libvmaf"
    echo "  --aomenc          Install only libaom (aomenc)"
    echo "  -j N              Number of parallel build threads (default: nproc)"
    echo "  --skip-check      Skip system dependencies check"
    echo "  -h, --help        Show this help"
    echo ""
    echo "EXAMPLES:"
    echo "  $0                    # Install all dependencies"
    echo "  $0 --ffmpeg          # Install FFmpeg (+ libopus, libvpx)"
    echo "  $0 -j 4             # Install all with 4 threads"
    echo "  $0 --svt-av1 --aomenc # Install SVT-AV1 and libaom"
}

#######################################
# Main
#######################################
main() {
    local install_all=true
    local install_opus=false
    local install_vpx=false
    local install_dav1d=false
    local install_vmaf=false
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
            --opus)
                install_all=false
                install_opus=true
                shift
                ;;
            --vpx)
                install_all=false
                install_vpx=true
                shift
                ;;
            --dav1d)
                install_all=false
                install_dav1d=true
                shift
                ;;
            --vmaf)
                install_all=false
                install_vmaf=true
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
            -j)
                NCPUS="$2"
                shift 2
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
        install_opus=true
        install_vpx=true
        install_dav1d=true
        install_vmaf=true
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

    # S'assurer que Meson/Ninja fonctionnent (nécessaire pour dav1d et libvmaf)
    if [[ "$install_dav1d" == true ]] || [[ "$install_vmaf" == true ]] || [[ "$install_ffmpeg" == true ]]; then
        if [[ "$PLATFORM" == "linux" ]]; then
            ensure_meson_ninja
            echo ""
        fi
    fi

    # S'assurer que CMake fonctionne (nécessaire pour SVT-AV1 et libaom)
    if [[ "$install_svt" == true ]] || [[ "$install_aom" == true ]]; then
        if [[ "$PLATFORM" == "linux" ]]; then
            ensure_cmake
            echo ""
        fi
    fi

    # libopus et libvpx doivent être compilées avant FFmpeg (dépendances)
    if [[ "$install_opus" == true ]] || [[ "$install_ffmpeg" == true ]]; then
        if [[ "$PLATFORM" == "linux" ]]; then
            install_opus
        fi
        echo ""
    fi

    if [[ "$install_vpx" == true ]] || [[ "$install_ffmpeg" == true ]]; then
        if [[ "$PLATFORM" == "linux" ]]; then
            install_libvpx
        fi
        echo ""
    fi

    # libdav1d doit être compilée avant FFmpeg (dépendance)
    if [[ "$install_dav1d" == true ]] || [[ "$install_ffmpeg" == true ]]; then
        if [[ "$PLATFORM" == "linux" ]]; then
            install_dav1d
        fi
        echo ""
    fi

    # libvmaf doit être compilée avant FFmpeg (dépendance)
    if [[ "$install_vmaf" == true ]] || [[ "$install_ffmpeg" == true ]]; then
        if [[ "$PLATFORM" == "linux" ]]; then
            install_vmaf
        fi
        echo ""
    fi

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
