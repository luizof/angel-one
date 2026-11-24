#!/usr/bin/env bash
# Prévia local persistente durante a sessão do usuário (Linux com systemd).
set -euo pipefail
cd "$(dirname "$0")"
port=18816
unit=angel-one-preview
tunnel=angel-one-tunnel
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/angel-one/target}"
case "${1:-status}" in
  build)
    (cd web && npm ci && npm run check && npm run build)
    cargo build --release --locked --manifest-path server/Cargo.toml
    ;;
  start)
    test -s web/dist/index.html && test -x "$CARGO_TARGET_DIR/release/angel-one" || {
      echo "Execute bash preview.sh build primeiro." >&2; exit 1;
    }
    data="${XDG_DATA_HOME:-$HOME/.local/share}/angel-one/preview"
    mkdir -p -m 700 "$data"
    if ! systemctl --user is-active --quiet "$unit"; then
      secret="${XDG_CONFIG_HOME:-$HOME/.config}/angel-one/preview.env"
      secret_opts=()
      if test -f "$secret"; then
        test "$(stat -c %a "$secret")" = 600 || { echo "Permissões inseguras em $secret (use chmod 600)" >&2; exit 1; }
        secret_opts=(--property="EnvironmentFile=$secret")
      fi
      systemd-run --user --collect --unit="$unit" "${secret_opts[@]}" --setenv=PORT="$port" \
        --setenv=DATA_DIR="$data" --setenv=WEB_DIR="$(realpath web/dist)" \
        "$CARGO_TARGET_DIR/release/angel-one"
    fi
    if ! systemctl --user is-active --quiet "$tunnel"; then
      systemd-run --user --collect --unit="$tunnel" "$(command -v cloudflared)" \
        tunnel --no-autoupdate --protocol http2 --url "http://127.0.0.1:$port"
    fi
    ;;
  status)
    systemctl --user is-active "$unit" "$tunnel" || true
    journalctl --user -u "$tunnel" --no-pager -o cat | grep -Eo 'https://[-a-z0-9]+\.trycloudflare\.com' | tail -1
    ;;
  stop)
    systemctl --user stop "$tunnel" "$unit"
    ;;
  *) echo "Uso: bash preview.sh build|start|status|stop"; exit 2 ;;
esac
