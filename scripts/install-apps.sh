#!/bin/sh
# [INPUT]: 当前平台与已构建 RioBaga-A/B、macOS/Linux 安装器。
# [OUTPUT]: 按平台转交安装；macOS 验签与启动，Linux 用户前缀安装并保留资料。
# [POS]: bun run install:apps 的兼容入口；安装职责统一交给 Python 标准库。
# [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
if [ "$(uname -s)" = Linux ]; then
  exec python3 "$(dirname "$0")/install-linux.py" "$@"
fi
exec python3 "${root}/scripts/install-apps.py" "$@"
