#!/usr/bin/env bash
# install-linux.sh — Instalación nativa de TheFoundry App en Linux
#
# Compila la app desde el código fuente, instala las dependencias del sistema
# y crea una entrada en el menú de aplicaciones para el usuario actual.
#
# Uso:
#   ./install-linux.sh             # instala
#   ./install-linux.sh --uninstall # desinstala
#   ./install-linux.sh --help

set -euo pipefail

# ── Configuración ────────────────────────────────────────────────────────────
APP_ID="thefoundry-app"
APP_NAME="The Foundry App"
APP_COMMENT="Lector de manga y reproductor de anime via torrents"

REPO_URL="https://github.com/madkyp/manga-reader.git"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Localizar el repo: dir del script → $HOME/manga-reader → clonar a ~/.cache
locate_repo() {
    for cand in "$SCRIPT_DIR" "$HOME/manga-reader" "$PWD"; do
        if [[ -f "$cand/package.json" && -d "$cand/src-tauri" ]]; then
            echo "$cand"; return 0
        fi
    done
    # No existe localmente: clonar
    local clone_dir="$HOME/.cache/thefoundry-app-src"
    if [[ ! -d "$clone_dir/.git" ]]; then
        info "Clonando código fuente en $clone_dir..." >&2
        command -v git >/dev/null 2>&1 || die "git no está instalado; ejecuta primero la instalación de deps o instala git manualmente"
        git clone --depth 1 "$REPO_URL" "$clone_dir" >&2
    else
        info "Actualizando código fuente en $clone_dir..." >&2
        git -C "$clone_dir" pull --ff-only >&2 || true
    fi
    echo "$clone_dir"
}

# REPO_DIR se establece en main() tras instalar deps (necesita git)
REPO_DIR=""

INSTALL_BIN="$HOME/.local/bin/$APP_ID"
INSTALL_DESKTOP="$HOME/.local/share/applications/$APP_ID.desktop"
INSTALL_ICONS="$HOME/.local/share/icons/hicolor"

# ── Output helpers ───────────────────────────────────────────────────────────
info()  { printf '\033[1;34m[*]\033[0m %s\n' "$*"; }
ok()    { printf '\033[1;32m[OK]\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m[!]\033[0m %s\n' "$*" >&2; }
die()   { printf '\033[1;31m[ERROR]\033[0m %s\n' "$*" >&2; exit 1; }

usage() {
    cat <<EOF
Uso: $0 [opción]

Opciones:
  (sin args)    Instala dependencias, compila y crea entrada de menú
  --uninstall   Elimina binario, icono y entrada de menú (no toca librerías)
  --help        Muestra esta ayuda

Distros soportadas para instalación de dependencias:
  Arch / CachyOS / Manjaro / EndeavourOS  (pacman)
  Debian / Ubuntu / Mint / Pop!_OS         (apt)
  Fedora / RHEL                            (dnf)
  openSUSE                                 (zypper)

En otras distros instala manualmente: webkit2gtk-4.1, gtk3,
libayatana-appindicator, librsvg, base-devel, ffmpeg, mpv, nodejs, npm.
EOF
}

# ── Detección de distro ──────────────────────────────────────────────────────
detect_distro() {
    [[ -f /etc/os-release ]] || die "No se encontró /etc/os-release; distro no detectada"
    # shellcheck source=/dev/null
    . /etc/os-release
    local id="${ID:-}" id_like="${ID_LIKE:-}"

    case "$id $id_like" in
        *arch*|*cachyos*|*manjaro*|*endeavouros*) echo "arch" ;;
        *debian*|*ubuntu*|*mint*|*pop*)           echo "debian" ;;
        *fedora*|*rhel*|*centos*)                 echo "fedora" ;;
        *opensuse*|*suse*)                        echo "opensuse" ;;
        *)                                        echo "unknown" ;;
    esac
}

# ── sudo ─────────────────────────────────────────────────────────────────────
SUDO=""
maybe_sudo() {
    if [[ $EUID -ne 0 ]]; then
        command -v sudo >/dev/null 2>&1 || die "sudo no disponible. Ejecuta como root o instala sudo."
        SUDO="sudo"
    fi
}

# ── Instalación de dependencias del sistema ──────────────────────────────────
install_deps() {
    local distro
    distro="$(detect_distro)"
    info "Distro detectada: $distro"

    case "$distro" in
        arch)
            maybe_sudo
            $SUDO pacman -S --needed --noconfirm \
                base-devel git curl wget pkgconf \
                webkit2gtk-4.1 gtk3 libayatana-appindicator librsvg \
                openssl nodejs npm \
                ffmpeg mpv
            ;;
        debian)
            maybe_sudo
            $SUDO apt-get update
            $SUDO apt-get install -y \
                build-essential git curl wget pkg-config file \
                libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
                libssl-dev nodejs npm \
                ffmpeg mpv
            ;;
        fedora)
            maybe_sudo
            $SUDO dnf install -y \
                @development-tools git curl wget pkgconf-pkg-config file \
                webkit2gtk4.1-devel gtk3-devel libayatana-appindicator-gtk3-devel librsvg2-devel \
                openssl-devel nodejs npm \
                ffmpeg mpv
            ;;
        opensuse)
            maybe_sudo
            $SUDO zypper install -y \
                -t pattern devel_basis
            $SUDO zypper install -y \
                git curl wget pkgconf \
                webkit2gtk3-devel gtk3-devel libayatana-appindicator3-devel librsvg-devel \
                libopenssl-devel nodejs npm \
                ffmpeg mpv
            ;;
        *)
            warn "Distro no reconocida; salta instalación automática."
            warn "Asegúrate de tener: webkit2gtk-4.1, gtk3, libayatana-appindicator,"
            warn "librsvg, base-devel, openssl, nodejs, npm, ffmpeg, mpv"
            read -rp "¿Continuar sin instalar dependencias? [s/N]: " resp
            [[ "$resp" =~ ^[sSyY]$ ]] || die "Cancelado por el usuario"
            ;;
    esac
    ok "Dependencias del sistema instaladas"
}

# ── Rust ─────────────────────────────────────────────────────────────────────
ensure_rust() {
    if command -v cargo >/dev/null 2>&1; then
        ok "Rust ya instalado ($(cargo --version))"
        return
    fi
    info "Rust no encontrado; instalando con rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    # shellcheck source=/dev/null
    . "$HOME/.cargo/env"
    ok "Rust instalado"
}

# ── Node ─────────────────────────────────────────────────────────────────────
ensure_node() {
    command -v node >/dev/null 2>&1 \
        || die "node no encontrado tras instalar dependencias; revisa el paquete nodejs"
    command -v npm >/dev/null 2>&1 \
        || die "npm no encontrado; instala el paquete npm"

    local node_major
    node_major="$(node -v | sed 's/^v//' | cut -d. -f1)"
    if (( node_major < 18 )); then
        die "Node $node_major es demasiado viejo (Tauri v2 requiere ≥18). Actualiza con nvm."
    fi
    ok "Node OK ($(node -v))"
}

# ── Build ────────────────────────────────────────────────────────────────────
build_app() {
    cd "$REPO_DIR"
    info "Repo: $REPO_DIR"
    info "Instalando deps de npm (puede tardar)..."
    npm ci 2>/dev/null || npm install
    info "Compilando la app en modo release (esto tarda varios minutos)..."
    # --bundles none: no genera AppImage/deb/rpm — solo el binario suelto
    npm run tauri build -- --bundles none
    local bin="$REPO_DIR/src-tauri/target/release/app"
    [[ -x "$bin" ]] || die "No se encontró el binario en $bin tras compilar"
    ok "Compilación completada"
}

# ── Instalación de archivos ──────────────────────────────────────────────────
install_files() {
    local bin_src="$REPO_DIR/src-tauri/target/release/app"
    local icon_dir="$REPO_DIR/src-tauri/icons"

    info "Instalando binario en $INSTALL_BIN"
    mkdir -p "$(dirname "$INSTALL_BIN")"
    install -m 0755 "$bin_src" "$INSTALL_BIN"

    info "Instalando iconos en $INSTALL_ICONS"
    for size in 32 64 128 256; do
        local src
        case $size in
            32)  src="$icon_dir/32x32.png" ;;
            64)  src="$icon_dir/64x64.png" ;;
            128) src="$icon_dir/128x128.png" ;;
            256) src="$icon_dir/128x128@2x.png" ;;
        esac
        if [[ -f "$src" ]]; then
            local dst="$INSTALL_ICONS/${size}x${size}/apps/$APP_ID.png"
            mkdir -p "$(dirname "$dst")"
            install -m 0644 "$src" "$dst"
        fi
    done

    info "Creando entrada de menú: $INSTALL_DESKTOP"
    mkdir -p "$(dirname "$INSTALL_DESKTOP")"
    cat > "$INSTALL_DESKTOP" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=$APP_NAME
GenericName=Manga & Anime
Comment=$APP_COMMENT
Exec=$INSTALL_BIN %U
Icon=$APP_ID
Terminal=false
Categories=AudioVideo;Video;Network;
StartupNotify=true
StartupWMClass=The Foundry APP
EOF
    chmod 0644 "$INSTALL_DESKTOP"

    # Refrescar caches del escritorio (silencioso si no existen las herramientas)
    command -v update-desktop-database >/dev/null 2>&1 \
        && update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 \
        && gtk-update-icon-cache -f "$INSTALL_ICONS" 2>/dev/null || true

    ok "Instalación completada"

    # Aviso si ~/.local/bin no está en PATH
    case ":$PATH:" in
        *":$HOME/.local/bin:"*) ;;
        *) warn "$HOME/.local/bin no está en PATH; el lanzador del menú funciona pero no podrás ejecutar '$APP_ID' desde la terminal." ;;
    esac
}

# ── Desinstalación ───────────────────────────────────────────────────────────
uninstall() {
    info "Eliminando archivos instalados..."
    rm -fv "$INSTALL_BIN" "$INSTALL_DESKTOP"
    for size in 32 64 128 256; do
        rm -fv "$INSTALL_ICONS/${size}x${size}/apps/$APP_ID.png"
    done

    command -v update-desktop-database >/dev/null 2>&1 \
        && update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 \
        && gtk-update-icon-cache -f "$INSTALL_ICONS" 2>/dev/null || true

    ok "Desinstalado. (Las librerías del sistema y Rust/Node se conservan.)"
}

# ── Main ─────────────────────────────────────────────────────────────────────
case "${1:-install}" in
    install)
        install_deps
        ensure_rust
        ensure_node
        REPO_DIR="$(locate_repo)"
        build_app
        install_files
        echo
        ok "Listo. Busca \"$APP_NAME\" en el menú de aplicaciones."
        ;;
    --uninstall|uninstall)
        uninstall
        ;;
    -h|--help|help)
        usage
        ;;
    *)
        usage
        exit 1
        ;;
esac
