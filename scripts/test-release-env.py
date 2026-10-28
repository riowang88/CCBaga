#!/usr/bin/env python3
# [INPUT]: 发布入口的真实 BUILD_ENV 与伪造环境凭据。
# [OUTPUT]: 构建子进程不能继承 Apple/更新凭据的回归断言。
# [POS]: macOS CI 的无真实凭据测试，验证环境隔离后才允许加载实际签名材料。
# [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
import importlib.util
import os
import json
import subprocess
import tempfile
from pathlib import Path
from unittest.mock import patch

credentials = ['APPLE_API_KEY', 'APPLE_API_ISSUER', 'APPLE_API_KEY_PATH',
               'APPLE_CERTIFICATE_PASSWORD', 'TAURI_SIGNING_PRIVATE_KEY',
               'TAURI_SIGNING_PRIVATE_KEY_PATH', 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD']
with patch.dict(os.environ, {key: 'disposable-fixture' for key in credentials}):
    spec = importlib.util.spec_from_file_location('release', Path(__file__).with_name('release-macos.py'))
    release = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(release)
    assert all(key not in release.BUILD_ENV for key in credentials)
    assert release.BUILD_ENV['PATH'] == os.environ['PATH']
print('Apple and updater credentials are excluded from build environments.')

# ----- 网络错误可恢复；无效票据和持续断网仍阻断交付 -----
network = subprocess.CompletedProcess([], 68, '', 'NSURLErrorDomain Code=-1005')
success = subprocess.CompletedProcess([], 0, '', '')
invalid = subprocess.CompletedProcess([], 65, '', 'Invalid ticket')
for results, expected, passes in [([network, success], 2, True), ([invalid], 1, False), ([network] * 3, 3, False)]:
    with patch.object(release.subprocess, 'run', side_effect=results) as command, patch.object(release.time, 'sleep'):
        try:
            release.staple('validate', Path('disposable-fixture.app'))
            assert passes
        except subprocess.CalledProcessError:
            assert not passes
        assert command.call_count == expected
        assert command.call_args.kwargs['env'] == release.BUILD_ENV
print('Ticket retries preserve validation failures and secret isolation.')

# 显式选择临时签名才能免 Apple 凭据；更新签名仍强制，默认公证不能静默降级。
with patch.dict(os.environ, {}, clear=True):
    assert release.signing_mode() == 'notarized'
    try:
        release.main()
        raise AssertionError('Missing Apple credentials were accepted')
    except SystemExit as error:
        assert 'CC_SIGN_IDENTITY' in str(error)
with patch.dict(os.environ, {'CC_MACOS_SIGNING_MODE': 'adhoc'}, clear=True):
    try:
        release.main()
        raise AssertionError('Unsigned updater was accepted')
    except SystemExit as error:
        assert 'updater signing private key' in str(error)
with patch.dict(os.environ, {'CC_MACOS_SIGNING_MODE': 'typo'}, clear=True):
    try:
        release.main()
        raise AssertionError('Unknown mode was accepted')
    except SystemExit as error:
        assert 'must be adhoc or notarized' in str(error)

# 真实入口 + 临时载荷 + 外部工具替身：不读取真实凭据，也不假称密码学验签通过。
with tempfile.TemporaryDirectory(prefix='ccbaga-release-mode-') as temporary:
    root = Path(temporary)
    (root / 'package.json').write_text('{"version":"1.2.3"}')
    host = root / 'CCBaga-A.app'
    host.mkdir()
    (host / 'fixture').write_text('disposable app payload')
    (root / 'browser-runtime').mkdir()
    (root / 'browser-runtime/Browser.zip').write_text('disposable engine archive')
    (root / 'src-tauri').mkdir()
    (root / 'src-tauri/tauri.conf.json').write_text(json.dumps({'plugins': {'updater': {'endpoints': ['https://github.com/riowang88/CCBaga/releases/latest/download/latest.json']}}}))
    commands = []
    def fake_run(*args, env=None):
        commands.append(tuple(map(str, args)))
        assert not any(key.startswith(('APPLE_', 'TAURI_SIGNING_PRIVATE_KEY')) for key in (env or release.BUILD_ENV))
        if args[:3] == ('bun', 'run', 'tauri'):
            assert json.loads(args[-1])['bundle']['macOS']['signingIdentity'] == '-'
    def fake_sign(args, **kwargs):
        assert args[:5] == ['bun', 'run', 'tauri', 'signer', 'sign']
        assert kwargs['env']['TAURI_SIGNING_PRIVATE_KEY'] == 'disposable-fixture'
        assert not any(key.startswith('APPLE_') for key in kwargs['env'])
        Path(args[-1] + '.sig').write_text('fixture-signature')
    def fake_dmg(path, *args, **kwargs):
        Path(path).write_text('disposable dmg')
    with patch.dict(os.environ, {'CC_MACOS_SIGNING_MODE': 'adhoc', 'CC_RELEASE_VERSION': '1.2.3', 'TAURI_SIGNING_PRIVATE_KEY': 'disposable-fixture'}, clear=True), patch.object(release, 'ROOT', root), patch.object(release, 'HOST', host), patch.object(release, 'BROWSER', root / 'Browser.app'), patch.object(release, 'run', side_effect=fake_run), patch.object(release.subprocess, 'run', side_effect=fake_sign), patch.object(release.dmgbuild, 'build_dmg', side_effect=fake_dmg):
        release.main()
    assert not any(cmd[0] in ('xcrun', 'spctl') for cmd in commands)
    assert any(cmd[:4] == ('codesign', '--verify', '--deep', '--strict') for cmd in commands)
    assert any('updater_tests::published_archive_matches_embedded_key_and_signed_version' in cmd for cmd in commands)
    assert any('scripts/verify-dmg.py' in cmd and '--adhoc' in cmd for cmd in commands)
    assert all(row['status'] == 'NotRequested' for row in json.loads((root / 'releases/notarization.json').read_text()))
    assert json.loads((root / 'releases/latest.json').read_text())['macos_signing'] == 'adhoc'
print('Explicit ad-hoc release retains payload checks and updater signing; default notarization stays required.')
