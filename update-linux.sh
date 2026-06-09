#!/usr/bin/env bash
# update-linux.sh — Actualiza TheFoundry App a la última versión del repo
#
# Trae los últimos cambios de Git, recompila el binario y reinstala el binario,
# los iconos y la entrada de menú. No toca las dependencias del sistema (usa
# install-linux.sh si hace falta instalarlas).
#
# Uso:
#   ./update-linux.sh           # comprueba, recompila si hay cambios y reinstala
#   ./update-linux.sh --check   # solo informa si hay actualizaciones (no compila)
#   ./update-linux.sh --force   # recompila e instala aunque ya esté al día
#   ./update-linux.sh --help

set -euo pipefail

# ── Configuración (debe coincidir con install-linux.sh) ──────────────────────
APP_ID="thefoundry-app"
APP_NAME="TheFoundry App"
APP_COMMENT="Lector de manga y reproductor de anime via torrents"

REPO_URL="https://github.com/madkyp/manga-reader.git"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

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
  (sin args)    Trae cambios de Git, recompila si hay novedades y reinstala
  --check       Solo comprueba si hay actualizaciones disponibles
  --force       Recompila e instala aunque ya esté al día
  --help        Muestra esta ayuda

Requiere que la app ya esté instalada (usa ./install-linux.sh la primera vez).
EOF
}

# ── Localizar el repo: dir del script → \$HOME/manga-reader → clon en ~/.cache ──
locate_repo() {
    for cand in "$SCRIPT_DIR" "$HOME/manga-reader" "$PWD"; do
        if [[ -f "$cand/package.json" && -d "$cand/src-tauri" && -d "$cand/.git" ]]; then
            echo "$cand"; return 0
        fi
    done
    local clone_dir="$HOME/.cache/thefoundry-app-src"
    if [[ -d "$clone_dir/.git" ]]; then
        echo "$clone_dir"; return 0
    fi
    die "No se encontró el repositorio con historial Git. Ejecuta ./install-linux.sh primero."
}

# ── Comprobaciones de toolchain (ligeras: solo verifican presencia) ──────────
check_toolchain() {
    command -v git   >/dev/null 2>&1 || die "git no está instalado"
    command -v node  >/dev/null 2>&1 || die "node no está instalado (usa install-linux.sh)"
    command -v npm   >/dev/null 2>&1 || die "npm no está instalado"
    if ! command -v cargo >/dev/null 2>&1; then
        # rustup instala cargo en ~/.cargo/bin; intentar cargarlo
        [[ -f "$HOME/.cargo/env" ]] && . "$HOME/.cargo/env"
    fi
    command -v cargo >/dev/null 2>&1 || die "cargo (Rust) no está instalado (usa install-linux.sh)"
}

pkg_version() { grep -m1 '"version"' "$REPO_DIR/package.json" | sed -E 's/.*"version"[^"]*"([^"]+)".*/\1/'; }

# ── Sincronizar con el remoto ────────────────────────────────────────────────
# Devuelve: 0 = hay cambios traídos, 1 = ya estaba al día
sync_repo() {
    local branch upstream
    branch="$(git -C "$REPO_DIR" rev-parse --abbrev-ref HEAD)"
    info "Repo:   $REPO_DIR (rama: $branch)" >&2

    if ! git -C "$REPO_DIR" diff --quiet || ! git -C "$REPO_DIR" diff --cached --quiet; then
        warn "Hay cambios locales sin commitear en el repo; se conservan (pull --ff-only)."
    fi

    info "Buscando actualizaciones en origin..." >&2
    git -C "$REPO_DIR" fetch --quiet origin "$branch" 2>/dev/null \
        || git -C "$REPO_DIR" fetch --quiet origin || warn "No se pudo hacer fetch (¿sin red?)"

    upstream="$(git -C "$REPO_DIR" rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null || true)"
    [[ -n "$upstream" ]] || { warn "La rama '$branch' no tiene upstream; no se puede comparar."; return 1; }

    local local_sha remote_sha
    local_sha="$(git -C "$REPO_DIR" rev-parse HEAD)"
    remote_sha="$(git -C "$REPO_DIR" rev-parse '@{u}')"
    if [[ "$local_sha" == "$remote_sha" ]]; then
        return 1
    fi

    info "Cambios nuevos en $upstream:" >&2
    git -C "$REPO_DIR" --no-pager log --oneline --no-decorate "HEAD..@{u}" 2>/dev/null | sed 's/^/    /' >&2 || true

    git -C "$REPO_DIR" pull --ff-only --quiet \
        || die "No se pudo hacer fast-forward (¿divergencia de ramas?). Resuélvelo manualmente con git."
    return 0
}

# ── Build (release, solo binario; igual que install-linux.sh) ────────────────
build_app() {
    cd "$REPO_DIR"
    info "Instalando deps de npm..."
    npm ci 2>/dev/null || npm install
    info "Compilando frontend (vite)..."
    npm run build
    info "Compilando binario release (puede tardar varios minutos)..."
    cargo build --release --features custom-protocol \
        --manifest-path "$REPO_DIR/src-tauri/Cargo.toml"
    [[ -x "$REPO_DIR/src-tauri/target/release/app" ]] \
        || die "No se encontró el binario tras compilar"
    ok "Compilación completada"
}

# ── Reinstalar binario + iconos + entrada de menú ────────────────────────────
install_files() {
    local bin_src="$REPO_DIR/src-tauri/target/release/app"
    local icon_dir="$REPO_DIR/src-tauri/icons"

    info "Actualizando binario en $INSTALL_BIN"
    mkdir -p "$(dirname "$INSTALL_BIN")"
    install -m 0755 "$bin_src" "$INSTALL_BIN"

    info "Actualizando iconos"
    for size in 16 32 64 128 256; do
        local src
        case $size in
            16)  src="$icon_dir/16x16.png" ;;
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

    info "Actualizando entrada de menú"
    mkdir -p "$(dirname "$INSTALL_DESKTOP")"
    cat > "$INSTALL_DESKTOP" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=$APP_NAME
GenericName=Manga & Anime
Comment=$APP_COMMENT
Exec=env WEBKIT_DISABLE_DMABUF_RENDERER=1 $INSTALL_BIN %U
Icon=$APP_ID
Terminal=false
Categories=AudioVideo;Video;Network;
StartupNotify=true
StartupWMClass=App
EOF
    chmod 0644 "$INSTALL_DESKTOP"

    command -v update-desktop-database >/dev/null 2>&1 \
        && update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 \
        && gtk-update-icon-cache -f "$INSTALL_ICONS" 2>/dev/null || true
}

# ── Avisar si la app está corriendo (usa el binario viejo hasta reiniciar) ────
warn_if_running() {
    if pgrep -x "app" >/dev/null 2>&1 || pgrep -f "$INSTALL_BIN" >/dev/null 2>&1; then
        warn "La app está en ejecución; ciérrala y vuelve a abrirla para aplicar la actualización."
    fi
}

# ── Main ─────────────────────────────────────────────────────────────────────
MODE="${1:-update}"
case "$MODE" in
    -h|--help|help) usage; exit 0 ;;
    --check|--force|update|"") ;;
    *) usage; exit 1 ;;
esac

check_toolchain
REPO_DIR="$(locate_repo)"

[[ -x "$INSTALL_BIN" ]] || warn "No hay binario instalado en $INSTALL_BIN; se instalará al actualizar."

ver_before="$(pkg_version || echo '?')"

if sync_repo; then
    has_updates=1
else
    has_updates=0
fi

if [[ "$MODE" == "--check" ]]; then
    if (( has_updates )); then
        ok "Hay una actualización disponible (versión local: $ver_before). Ejecuta '$0' para aplicarla."
    else
        ok "Ya estás en la última versión ($ver_before)."
    fi
    exit 0
fi

if (( has_updates == 0 )) && [[ "$MODE" != "--force" ]] && [[ -x "$INSTALL_BIN" ]]; then
    ok "Ya estás en la última versión ($ver_before). Usa --force para reinstalar igualmente."
    exit 0
fi

build_app
install_files
warn_if_running

ver_after="$(pkg_version || echo '?')"
echo
if [[ "$ver_before" != "$ver_after" ]]; then
    ok "Actualizado: $ver_before → $ver_after"
else
    ok "Reinstalado (versión $ver_after)"
fi
ok "Listo. Reinicia la app si estaba abierta."
