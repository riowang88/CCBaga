#!/bin/bash
# [INPUT]: 固定摘要的 Arch CI 容器、已检出源码提交与 CC_RELEASE_VERSION。
# [OUTPUT]: 原生构建/测试并解包验收的 pacman 安装包及独立 SHA256 清单。
# [POS]: Linux 发布隔离层；非 root 编译，没有 Apple 或更新凭据，不更改用户资料。
# [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
set -euo pipefail
cd /work
pacman -Syu --noconfirm --needed bun nodejs rust git pkgconf python cmake webkit2gtk-4.1 gtk3 libayatana-appindicator librsvg patchelf zsh openssh xdg-utils curl nss nspr mesa alsa-lib libxss
useradd -m -u "$(stat -c '%u' /work)" builder
chown -R builder:builder packaging/arch
runuser -u builder -- env CARGO_TARGET_DIR=/work/src-tauri/target CC_RELEASE_VERSION="$CC_RELEASE_VERSION" CCBAGA_GIT_URL=file:///work CCBAGA_GIT_REF="commit=$GITHUB_SHA" bash -c 'cd /work/packaging/arch; makepkg -f'
package=$(find packaging/arch -maxdepth 1 -name '*.pkg.tar.zst' -type f)
test -n "$package"
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT
bsdtar -xf "$package" -C "$temporary"
python scripts/verify-linux-package.py "$temporary" packaging/arch/src/ccbaga/browser-runtime/LinuxBrowser/chrome
grep -qx "pkgver = $CC_RELEASE_VERSION-1" "$temporary/.PKGINFO"
mkdir -p releases
target="RioBaga-$CC_RELEASE_VERSION-linux-x86_64.pkg.tar.zst"
cp "$package" "releases/$target"
(cd releases; sha256sum "$target" > Arch-SHA256SUMS)
