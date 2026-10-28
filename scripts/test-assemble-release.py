#!/usr/bin/env python3
# [INPUT]: 真实发布装配入口与临时平台产物/校验清单，无真实安装器或签名材料。
# [OUTPUT]: 本仓库九文件交付、旧更新源拒绝和缺失平台摘要时拒绝未验收产物的离线回归。
# [POS]: 发布合并边界的标准库测试，不发布、不下载、不操作用户文件。
# [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('assembly', Path(__file__).with_name('assemble-release.py'))
assembly = importlib.util.module_from_spec(spec)
spec.loader.exec_module(assembly)


class ReleaseAssemblyTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='ccbaga-assembly-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.out = self.root / 'releases'
        self.out.mkdir()
        self.version = '1.2.3'
        prefix = f'CCBaga-{self.version}'
        self.dmg = prefix + '-macos-arm64.dmg'
        self.archive = prefix + '-macos-arm64.app.tar.gz'
        self.windows = prefix + '-windows-x86_64-setup.exe'
        self.linux = [prefix + '-linux-x86_64.' + ext for ext in ['deb', 'rpm', 'AppImage']]
        self.arch = prefix + '-linux-x86_64.pkg.tar.zst'
        for name in [self.dmg, self.archive, self.windows, *self.linux, self.arch]:
            (self.out / name).write_bytes(b'disposable verified artifact')
        for name, platform, manifest in [(self.archive, 'darwin-aarch64', 'latest.json'),
                                          (self.windows, 'windows-x86_64', 'latest-windows.json')]:
            signature = 'disposable-signature-' + platform
            (self.out / (name + '.sig')).write_text(signature + '\n')
            (self.out / manifest).write_text(json.dumps({'version': self.version, 'platforms': {
                platform: {'url': f'https://github.com/riowang88/CCBaga/releases/download/v{self.version}/{name}',
                           'signature': signature}}}))
        self.lists = {'SHA256SUMS': [self.dmg, self.archive, self.archive + '.sig', 'latest.json'],
                      'Linux-SHA256SUMS': self.linux,
                      'Arch-SHA256SUMS': [self.arch], 'Windows-SHA256SUMS': [self.windows]}
        self.write_lists()
        self.addCleanup(patch.stopall)
        patch.object(assembly, 'ROOT', self.root).start()
        patch.object(assembly, 'OUT', self.out).start()
        patch.dict(os.environ, {'RELEASE_VERSION': self.version}).start()

    def write_lists(self):
        for name, artifacts in self.lists.items():
            lines = [hashlib.sha256((self.out / artifact).read_bytes()).hexdigest() + '  ' + artifact + '\n'
                     for artifact in artifacts]
            (self.out / name).write_text(''.join(lines))

    def test_complete_inputs_publish_only_nine_verified_assets(self):
        assembly.main()
        published = (self.root / 'release-files.txt').read_text().splitlines()
        expected = [self.dmg, self.windows, *self.linux, self.arch, self.archive, 'latest.json', 'SHA256SUMS']
        self.assertEqual(published, ['releases/' + name for name in expected])
        for line in (self.out / 'SHA256SUMS').read_text().splitlines():
            digest, filename = line.split('  ', 1)
            self.assertEqual(digest, hashlib.sha256((self.out / filename).read_bytes()).hexdigest())

    def test_old_repository_update_url_is_rejected(self):
        for filename, platform in [('latest.json', 'darwin-aarch64'),
                                   ('latest-windows.json', 'windows-x86_64')]:
            with self.subTest(platform=platform):
                path = self.out / filename
                original = path.read_bytes()
                manifest = json.loads(original)
                manifest['platforms'][platform]['url'] = manifest['platforms'][platform]['url'].replace(
                    'github.com/riowang88/CCBaga/', 'github.com/liseami/CCBaga/')
                path.write_text(json.dumps(manifest))
                self.write_lists()
                try:
                    with self.assertRaisesRegex(SystemExit, 'Updater URL/signature mismatch'):
                        assembly.main()
                    self.assertFalse((self.root / 'release-files.txt').exists())
                finally:
                    path.write_bytes(original)
                    self.write_lists()

    def test_adhoc_notice_preserves_verified_updater(self):
        path = self.out / 'latest.json'
        manifest = json.loads(path.read_text())
        manifest['macos_signing'] = 'adhoc'
        expected = manifest['platforms']['darwin-aarch64'].copy()
        path.write_text(json.dumps(manifest))
        self.write_lists()
        assembly.main()
        self.assertEqual(json.loads(path.read_text())['platforms']['darwin-aarch64'], expected)
        self.assertIn('not notarized by Apple', (self.root / 'release-notes.md').read_text())

    def test_omitted_artifact_cannot_be_rehashed_and_published(self):
        for manifest, artifact in [('SHA256SUMS', self.dmg), ('Windows-SHA256SUMS', self.windows),
                                   *[('Linux-SHA256SUMS', name) for name in self.linux],
                                   ('Arch-SHA256SUMS', self.arch)]:
            with self.subTest(manifest=manifest, artifact=artifact):
                originals = {name: (self.out / name).read_bytes() for name in [*self.lists, 'latest.json', artifact]}
                self.lists[manifest].remove(artifact)
                self.write_lists()
                (self.out / artifact).write_bytes(b'unverified replacement after platform checks')
                original_latest = (self.out / 'latest.json').read_bytes()
                try:
                    with self.assertRaisesRegex(SystemExit, 'Missing verified artifact'):
                        assembly.main()
                    self.assertFalse((self.root / 'release-files.txt').exists())
                    self.assertFalse((self.root / 'release-notes.md').exists())
                    self.assertEqual((self.out / 'latest.json').read_bytes(), original_latest)
                finally:
                    self.lists[manifest].append(artifact)
                    for name, data in originals.items():
                        (self.out / name).write_bytes(data)
                    for name in ['release-files.txt', 'release-notes.md']:
                        (self.root / name).unlink(missing_ok=True)


if __name__ == '__main__':
    unittest.main()
