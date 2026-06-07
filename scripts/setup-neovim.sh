#!/usr/bin/env bash
#
# setup-neovim.sh — Configure Neovim with CodeCompanion.nvim + zerostack ACP
#
# Usage:
#   bash setup-neovim.sh
#
#   # Skip zerostack install check:
#   bash setup-neovim.sh --skip-zerostack
#
set -euo pipefail

SKIP_ZEROSTACK=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-zerostack)
            SKIP_ZEROSTACK=true
            shift
            ;;
        --help|-h)
            cat <<EOF
Usage: setup-neovim.sh [--skip-zerostack]

Options:
  --skip-zerostack   Skip the zerostack installation check
  --help, -h         Show this message
EOF
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

NVIM_CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/nvim"
LAZY_PATH="${NVIM_CONFIG_DIR}/lazy-lock.json"
CC_DIR="${NVIM_CONFIG_DIR}/lua/plugins"
CC_FILE="${CC_DIR}/codecompanion.lua"

echo "=== zerostack + Neovim ACP Setup ==="

# ---- 1. Check neovim ----
if ! command -v nvim &>/dev/null; then
    echo "Error: neovim not found. Install it first:" >&2
    echo "  https://github.com/neovim/neovim/blob/master/INSTALL.md" >&2
    exit 1
fi

NVIM_VERSION=$(nvim --version | head -1 | grep -oP '\d+\.\d+' | head -1 || echo "0.0")
echo "Found neovim ${NVIM_VERSION}"

# ---- 2. Check zerostack ----
if ! $SKIP_ZEROSTACK; then
    if command -v zerostack &>/dev/null; then
        echo "Found zerostack at $(which zerostack)"
        if ! zerostack --help 2>&1 | grep -q '\-\-acp'; then
            echo "WARNING: This zerostack build does not include ACP support." >&2
            echo "  Rebuild with: cargo install zerostack --features acp" >&2
        fi
    else
        echo "zerostack not found in PATH." >&2
        echo "  Install from source: cargo install zerostack --features acp" >&2
        echo "  Or run with --skip-zerostack to skip this check." >&2
        exit 1
    fi
fi

# ---- 3. Create neovim config directories ----
mkdir -p "$NVIM_CONFIG_DIR" "$CC_DIR"

# ---- 4. Bootstrap lazy.nvim if not present ----
if [[ ! -f "$LAZY_PATH" ]]; then
    echo "Bootstrapping lazy.nvim..."
    cat > "${NVIM_CONFIG_DIR}/init.lua" <<'LAZYEOF'
-- Bootstrap lazy.nvim
local lazypath = vim.fn.stdpath("data") .. "/lazy/lazy.nvim"
if not (vim.uv or vim.loop).fs_stat(lazypath) then
    vim.fn.system({
        "git",
        "clone",
        "--filter=blob:none",
        "--branch=stable",
        "https://github.com/folke/lazy.nvim.git",
        lazypath,
    })
end
vim.opt.rtp:prepend(lazypath)

require("lazy").setup("plugins")

-- CodeCompanion keymaps
vim.api.nvim_set_keymap("n", "<leader>cc", ":CodeCompanionChat<CR>", { noremap = true, silent = true, desc = "Chat" })
vim.api.nvim_set_keymap("v", "<leader>cc", ":CodeCompanionChat<CR>", { noremap = true, silent = true, desc = "Chat" })
vim.api.nvim_set_keymap("n", "<leader>ca", ":CodeCompanionActions<CR>", { noremap = true, silent = true, desc = "Actions" })
vim.api.nvim_set_keymap("v", "<leader>ca", ":CodeCompanionActions<CR>", { noremap = true, silent = true, desc = "Actions" })
LAZYEOF
else
    echo "lazy.nvim already bootstrapped."
fi

# ---- 5. Write codecompanion.nvim plugin spec ----
cat > "$CC_FILE" <<'CCEOF'
return {
    "olimorris/codecompanion.nvim",
    dependencies = {
        "nvim-lua/plenary.nvim",
        "nvim-treesitter/nvim-treesitter",
    },
    config = function()
        require("codecompanion").setup({
            adapters = {
                acp = {
                    opts = {
                        show_presets = false,
                    },
                    zerostack = function()
                        return {
                            name = "zerostack",
                            formatted_name = "Zerostack",
                            type = "acp",
                            roles = {
                                llm = "assistant",
                                user = "user",
                            },
                            opts = {
                                vision = false,
                            },
                            commands = {
                                default = {
                                    "zerostack",
                                    "--acp",
                                },
                            },
                            defaults = {
                                mcpServers = {},
                                timeout = 60000,
                            },
                            parameters = {
                                protocolVersion = 1,
                                clientCapabilities = {
                                    fs = { readTextFile = true, writeTextFile = true },
                                },
                                clientInfo = {
                                    name = "CodeCompanion.nvim",
                                    version = "1.0.0",
                                },
                            },
                            handlers = {
                                setup = function(self)
                                    return true
                                end,
                                auth = function(self)
                                    return true
                                end,
                                form_messages = function(self, messages, capabilities)
                                    return require("codecompanion.adapters.acp.helpers")
                                        .form_messages(self, messages, capabilities)
                                end,
                                on_exit = function(self, code) end,
                            },
                        }
                    end,
                },
            },
            interactions = {
                chat = {
                    adapter = "zerostack",
                },
            },
            display = {
                chat = {
                    show_header_separator = false,
                },
            },
        })
    end,
}
CCEOF

echo ""
echo "=== Setup complete ==="
echo ""
echo "Configuration written to:"
echo "  ${NVIM_CONFIG_DIR}/init.lua"
echo "  ${CC_FILE}"
echo ""
echo "Next steps:"
echo "  1. Launch neovim and let lazy.nvim install plugins"
echo "  2. Open a chat with: <leader>cc (default: \\cc)"
echo "  3. Select the 'zerostack' adapter in the chat buffer"
echo ""
echo "Ensure zerostack is built with ACP support:"
echo "  cargo install zerostack --features acp"
echo ""
echo "You also need a provider configured (API keys, model, etc.)."
echo "See: zerostack --help"
