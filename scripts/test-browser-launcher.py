#!/usr/bin/env python3
# [INPUT]: browser-runtime/launcher/host_lookup.h 与 CCBrowser.c、系统 clang/CoreFoundation、临时伪应用包。
# [OUTPUT]: 编译 CCBrowser 入口确认可构建；用临时目录验证 B 找 A 的候选顺序：仅新名、仅旧名、两者都有时新名优先、名字对但 Bundle ID 不符时拒绝、都没有时返回 0。
# [POS]: macOS 入口宿主查找的隔离回归；不运行真实入口、不打开应用、不访问 /Applications。非 macOS 跳过。
# [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
import platform
import plistlib
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LAUNCHER = ROOT / 'browser-runtime/launcher'
HOST_ID = 'ai.local.requirement-french'
PROBE = r'''
#include <limits.h>
#include "host_lookup.h"
int main(int argc, char **argv) {
    char out[PATH_MAX];
    if (!find_host_app(argv[1], argv[2], out, sizeof(out))) { puts(""); return 3; }
    puts(out);
    return 0;
}
'''


def fake_app(folder: Path, name: str, identifier: str | None) -> Path:
    app = folder / name
    (app / 'Contents/MacOS').mkdir(parents=True)
    if identifier is not None:
        with open(app / 'Contents/Info.plist', 'wb') as handle:
            plistlib.dump({'CFBundleIdentifier': identifier, 'CFBundleExecutable': 'requirement-french'}, handle)
    return app


def main() -> None:
    if platform.system() != 'Darwin':
        print('skip: macOS launcher only')
        return
    with tempfile.TemporaryDirectory() as temporary:
        work = Path(temporary)
        probe_src = work / 'probe.c'
        probe_src.write_text(PROBE)
        probe = work / 'probe'
        base = ['/usr/bin/clang', '-O2', '-Wall', '-Werror', '-arch', platform.machine(), '-framework', 'CoreFoundation', '-I', str(LAUNCHER)]
        subprocess.run(base + ['-o', str(probe), str(probe_src)], check=True)
        subprocess.run(base + ['-o', str(work / 'CCBrowser'), str(LAUNCHER / 'CCBrowser.c')], check=True)

        counter = iter(range(100))

        def scenario() -> Path:
            # 每个场景独立目录：CFBundle 按 URL 缓存，复用同一路径会读到旧标识。
            folder = work / f'case-{next(counter)}'
            folder.mkdir()
            return folder

        def find(folder: Path) -> str:
            result = subprocess.run([str(probe), str(folder), HOST_ID], capture_output=True, text=True)
            assert result.returncode in (0, 3), result
            return result.stdout.strip()

        failures = []

        def check(label: str, actual: str, expected: str) -> None:
            ok = actual == expected
            print(('ok   ' if ok else 'FAIL ') + label)
            if not ok:
                failures.append(f'{label}: expected {expected!r}, got {actual!r}')

        folder = scenario()
        check('none -> not found', find(folder), '')
        folder = scenario()
        new = fake_app(folder, 'RioBaga-A.app', HOST_ID)
        check('only new name', find(folder), str(new))
        folder = scenario()
        old = fake_app(folder, 'CCBaga-A.app', HOST_ID)
        check('only legacy name', find(folder), str(old))
        folder = scenario()
        new = fake_app(folder, 'RioBaga-A.app', HOST_ID)
        fake_app(folder, 'CCBaga-A.app', HOST_ID)
        check('both -> new name first', find(folder), str(new))
        folder = scenario()
        fake_app(folder, 'RioBaga-A.app', 'com.example.other')
        fake_app(folder, 'CCBaga-A.app', 'ai.local.requirement-french.browser')
        check('right names, wrong bundle ids -> rejected', find(folder), '')
        folder = scenario()
        fake_app(folder, 'RioBaga-A.app', 'com.example.other')
        old = fake_app(folder, 'CCBaga-A.app', HOST_ID)
        check('wrong new id falls through to valid legacy', find(folder), str(old))
        folder = scenario()
        fake_app(folder, 'RioBaga-A.app', None)
        check('missing Info.plist -> rejected', find(folder), '')
        folder = scenario()
        (folder / 'RioBaga-A.app').write_text('not a bundle')
        check('plain file named like app -> rejected', find(folder), '')
        if failures:
            print('\n'.join(failures), file=sys.stderr)
            sys.exit(1)
        print('browser launcher host lookup: 8 passed')


if __name__ == '__main__':
    main()
