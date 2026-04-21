#!/bin/sh
set -eu

REPO_URL="${MUXWF_REPO:-https://github.com/tuannm99/muxfw.git}"
BRANCH="${MUXWF_BRANCH:-master}"
INSTALL_DIR="${MUXWF_INSTALL_DIR:-$HOME/.local/src/muxwf}"
INSTALL_ROOT="${MUXWF_INSTALL_ROOT:-$HOME/.local}"

info() {
    printf '%s\n' "muxwf: $*"
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1
}

sudo_cmd() {
    if [ "$(id -u)" -eq 0 ]; then
        return 0
    fi
    if need_cmd sudo; then
        printf '%s' "sudo"
        return 0
    fi
    printf '%s\n' "muxwf: sudo is required to install apt packages" >&2
    exit 1
}

install_ubuntu_packages() {
    if ! need_cmd apt-get; then
        info "apt-get not found; skipping system package install"
        return 0
    fi

    SUDO="$(sudo_cmd)"
    info "installing Ubuntu packages"
    if [ -n "$SUDO" ]; then
        $SUDO apt-get update
        $SUDO env DEBIAN_FRONTEND=noninteractive apt-get install -y \
            ca-certificates curl git build-essential pkg-config tmux fzf
    else
        apt-get update
        DEBIAN_FRONTEND=noninteractive apt-get install -y \
            ca-certificates curl git build-essential pkg-config tmux fzf
    fi
}

install_rust() {
    if need_cmd cargo; then
        return 0
    fi
    if ! need_cmd curl; then
        printf '%s\n' "muxwf: curl is required to install Rust" >&2
        exit 1
    fi

    info "installing Rust with rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
}

checkout_repo() {
    parent_dir="$(dirname "$INSTALL_DIR")"
    mkdir -p "$parent_dir"

    if [ -d "$INSTALL_DIR/.git" ]; then
        info "updating $INSTALL_DIR"
        git -C "$INSTALL_DIR" fetch --depth 1 origin "$BRANCH"
        git -C "$INSTALL_DIR" checkout -q -B "$BRANCH" "origin/$BRANCH"
        return 0
    fi

    if [ -e "$INSTALL_DIR" ]; then
        printf '%s\n' "muxwf: $INSTALL_DIR exists but is not a git checkout" >&2
        exit 1
    fi

    info "cloning $REPO_URL"
    git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$INSTALL_DIR"
}

install_muxwf() {
    mkdir -p "$INSTALL_ROOT/bin"
    info "installing muxwf into $INSTALL_ROOT/bin"
    cargo install --path "$INSTALL_DIR" --root "$INSTALL_ROOT" --locked --force
    ln -sf "$INSTALL_ROOT/bin/muxwf" "$INSTALL_ROOT/bin/mw"
}

install_completions() {
    muxwf_bin="$INSTALL_ROOT/bin/muxwf"
    if [ ! -x "$muxwf_bin" ]; then
        return 0
    fi
    if ! "$muxwf_bin" completion zsh >/dev/null 2>&1; then
        info "completion command not available in installed binary; skipping shell completions"
        return 0
    fi

    info "installing shell completions"
    mkdir -p "$HOME/.local/share/bash-completion/completions"
    "$muxwf_bin" completion bash --name muxwf > "$HOME/.local/share/bash-completion/completions/muxwf"
    "$muxwf_bin" completion bash > "$HOME/.local/share/bash-completion/completions/mw"

    mkdir -p "$HOME/.local/share/zsh/site-functions"
    "$muxwf_bin" completion zsh --name muxwf > "$HOME/.local/share/zsh/site-functions/_muxwf"
    "$muxwf_bin" completion zsh > "$HOME/.local/share/zsh/site-functions/_mw"

    mkdir -p "$HOME/.config/fish/completions"
    "$muxwf_bin" completion fish --name muxwf > "$HOME/.config/fish/completions/muxwf.fish"
    "$muxwf_bin" completion fish > "$HOME/.config/fish/completions/mw.fish"

    if [ -f "$HOME/.zcompdump" ]; then
        rm -f "$HOME/.zcompdump" "$HOME"/.zcompdump-*
    fi
}

install_zsh_fpath() {
    zshenv="$HOME/.zshenv"
    marker="# muxwf zsh completion"
    if [ -f "$zshenv" ] && grep -Fq "$marker" "$zshenv"; then
        return 0
    fi

    {
        printf '\n%s\n' "$marker"
        printf '%s\n' 'if [ -d "$HOME/.local/share/zsh/site-functions" ]; then'
        printf '%s\n' '  fpath=("$HOME/.local/share/zsh/site-functions" $fpath)'
        printf '%s\n' 'fi'
    } >> "$zshenv"
}

install_neovim_plugin() {
    if ! need_cmd nvim; then
        info "nvim not found; skipping Neovim plugin install"
        return 0
    fi
    plugin_dir="$HOME/.config/nvim/pack/muxwf/start/muxwf.nvim"
    mkdir -p "$plugin_dir/plugin"
    rm -f "$plugin_dir/plugin/muxwf.vim"
    cp "$INSTALL_DIR/nvim/plugin/muxwf.lua" "$plugin_dir/plugin/muxwf.lua"
    info "installed Neovim plugin at $plugin_dir"
}

print_next_steps() {
    info "installed: $INSTALL_ROOT/bin/muxwf and $INSTALL_ROOT/bin/mw"
    if ! printf '%s' ":$PATH:" | grep -q ":$INSTALL_ROOT/bin:"; then
        info "add this to your shell profile:"
        printf '%s\n' "export PATH=\"$INSTALL_ROOT/bin:\$PATH\""
    fi
    if [ -f "$HOME/.local/share/zsh/site-functions/_mw" ]; then
        info "zsh completions installed at $HOME/.local/share/zsh/site-functions/_mw"
    fi
    if [ -f "$HOME/.config/nvim/pack/muxwf/start/muxwf.nvim/plugin/muxwf.lua" ]; then
        info "Neovim commands: :MwOpen, :MwJump, :MwWorkspaceOpen, :MwWorkspaceList"
    fi
    info "restart zsh with: exec zsh"
    info "run: mw doctor"
}

install_ubuntu_packages
install_rust
checkout_repo
install_muxwf
install_completions
install_zsh_fpath
install_neovim_plugin
print_next_steps
