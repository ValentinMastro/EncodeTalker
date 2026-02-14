#!/bin/bash
# Script de cross-compilation pour Windows

set -e

echo "🔨 Cross-compilation pour Windows (x86_64-pc-windows-gnu)..."
echo ""

# Vérifier que le target est installé
if ! rustc --print target-list | grep -q "x86_64-pc-windows-gnu"; then
    echo "❌ Target x86_64-pc-windows-gnu non disponible"
    echo "Installez-le avec: rustup target add x86_64-pc-windows-gnu"
    exit 1
fi

# Vérifier que MinGW est installé
if ! command -v x86_64-w64-mingw32-gcc &> /dev/null; then
    echo "❌ MinGW-w64 non installé"
    echo "Installez-le avec: sudo pacman -S mingw-w64-gcc mingw-w64-binutils mingw-w64-crt"
    exit 1
fi

echo "✅ Toolchain disponible"
echo ""

# Compilation
echo "📦 Compilation en cours..."
cargo build --release --target x86_64-pc-windows-gnu "$@"

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Compilation réussie !"
    echo ""
    echo "📁 Binaires Windows disponibles dans:"
    echo "   target/x86_64-pc-windows-gnu/release/encodetalker-daemon.exe"
    echo "   target/x86_64-pc-windows-gnu/release/encodetalker-tui.exe"
    echo ""
    echo "💡 Pour tester sur Windows:"
    echo "   1. Copiez ces fichiers sur une machine Windows"
    echo "   2. Lancez encodetalker-tui.exe"
    echo "   3. Le daemon se lancera automatiquement"
else
    echo ""
    echo "❌ Échec de la compilation"
    exit 1
fi
