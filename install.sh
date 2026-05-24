#!/bin/sh
set -e

REPO="gkanellopoulos/dcal"
INSTALL_DIR="${DCAL_INSTALL_DIR:-/usr/local/bin}"

main() {
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin) os_target="apple-darwin" ;;
        Linux)  os_target="unknown-linux-gnu" ;;
        *)
            echo "Error: unsupported OS: $os" >&2
            exit 1
            ;;
    esac

    case "$arch" in
        arm64|aarch64) arch_target="aarch64" ;;
        x86_64)        arch_target="x86_64" ;;
        *)
            echo "Error: unsupported architecture: $arch" >&2
            exit 1
            ;;
    esac

    target="${arch_target}-${os_target}"
    filename="dcal-${target}.tar.gz"

    # Get latest release tag
    tag="$(curl -sI "https://github.com/${REPO}/releases/latest" \
        | grep -i '^location:' \
        | sed 's/.*\/tag\///' \
        | tr -d '\r\n')"

    if [ -z "$tag" ]; then
        echo "Error: could not determine latest release" >&2
        exit 1
    fi

    url="https://github.com/${REPO}/releases/download/${tag}/${filename}"

    echo "Installing dcal ${tag} (${target})..."

    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    curl -sL "$url" -o "${tmpdir}/${filename}"
    tar xzf "${tmpdir}/${filename}" -C "$tmpdir"

    if [ ! -f "${tmpdir}/dcal" ]; then
        echo "Error: binary not found in archive. Is ${target} supported?" >&2
        exit 1
    fi

    if [ -w "$INSTALL_DIR" ]; then
        mv "${tmpdir}/dcal" "${INSTALL_DIR}/dcal"
    else
        echo "Installing to ${INSTALL_DIR} (requires sudo)..."
        sudo mv "${tmpdir}/dcal" "${INSTALL_DIR}/dcal"
    fi

    chmod +x "${INSTALL_DIR}/dcal"

    echo "Installed dcal ${tag} to ${INSTALL_DIR}/dcal"
    echo "Run 'dcal init' to get started."
}

main
