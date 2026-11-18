#!/usr/bin/env python3
# [INPUT]: 可丢弃构建树、安装前缀、Linux 安装器、真实产物验收层和 Linux GIO 桌面解析器。
# [OUTPUT]: 安装边界和真实 GIO 特殊字符/%/= 路径往返、无 shell 执行与继承优先级不变；非 Linux 明确跳过 GIO。
# [POS]: 离线临时目录测试，不安装到真实用户目录。
# [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('install_linux', Path(__file__).with_name('install-linux.py'))
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)
payload_spec = importlib.util.spec_from_file_location('payload', Path(__file__).with_name('verify-linux-package.py'))
payload = importlib.util.module_from_spec(payload_spec)
payload_spec.loader.exec_module(payload)


class InstallTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='ccbaga-test-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.build = self.root / 'build'
        self.prefix = self.root / 'prefix with spaces'
        for name in ['requirement-french', 'browser-runtime/LinuxBrowser/chrome', 'browser-ui/browser.html',
                     'browser-ui/assets/browser.js', 'browser-ui/assets/locales.js', 'browser-ui/assets/browser.css']:
            path = self.build / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('fixture')
            path.chmod(0o755)
        (self.build / 'browser-ui/browser.html').write_text(
            '<script type="module" src="./assets/browser.js"></script>'
            '<link rel="modulepreload" href="./assets/locales.js">'
            '<link rel="stylesheet" href="./assets/browser.css">')

    def test_install_upgrade_preserves_unrelated_user_data(self):
        data = self.prefix / 'share/ai.local.requirement-french/browser/profile/keep'
        data.parent.mkdir(parents=True)
        data.write_text('personal')
        installer.install(self.build, self.prefix)
        installer.install(self.build, self.prefix)
        self.assertEqual(data.read_text(), 'personal')
        self.assertEqual((self.prefix / 'bin/ccbaga-a').read_text(), 'fixture')
        self.assertIn('--browser %U', (self.prefix / 'share/applications/ccbaga-browser.desktop').read_text())
        self.assertTrue((self.prefix / 'share/ccbaga-install/lib/CCBaga-A/browser-runtime/LinuxBrowser/chrome').exists())

    def test_custom_launcher_and_linked_desktop_are_preserved(self):
        launcher = self.prefix / 'bin/ccbaga-a'
        launcher.parent.mkdir(parents=True)
        launcher.write_text('custom')
        with self.assertRaises(RuntimeError):
            installer.install(self.build, self.prefix)
        self.assertEqual(launcher.read_text(), 'custom')
        launcher.unlink()
        desktop = self.prefix / 'share/applications/ccbaga-a.desktop'
        desktop.parent.mkdir(parents=True)
        desktop.symlink_to(launcher)
        with self.assertRaises(RuntimeError):
            installer.install(self.build, self.prefix)
        self.assertTrue(desktop.is_symlink())

    def test_non_executable_browser_preserves_existing_installation(self):
        installer.install(self.build, self.prefix)
        launcher = self.prefix / 'bin/ccbaga-a'
        desktop = self.prefix / 'share/applications/ccbaga-browser.desktop'
        previous_desktop = desktop.read_bytes()
        (self.build / 'requirement-french').write_text('replacement')
        (self.build / 'browser-runtime/LinuxBrowser/chrome').chmod(0o644)
        with self.assertRaisesRegex(RuntimeError, 'browser'):
            installer.install(self.build, self.prefix)
        self.assertEqual(launcher.read_text(), 'fixture')
        self.assertEqual(desktop.read_bytes(), previous_desktop)
        browser = self.prefix / 'share/ccbaga-install/lib/CCBaga-A/browser-runtime/LinuxBrowser/chrome'
        self.assertTrue(installer.os.access(browser, installer.os.X_OK))

    def test_incomplete_browser_ui_preserves_existing_installation(self):
        installer.install(self.build, self.prefix)
        launcher = self.prefix / 'bin/ccbaga-a'
        desktop = self.prefix / 'share/applications/ccbaga-browser.desktop'
        previous_desktop = desktop.read_bytes()
        (self.build / 'requirement-french').write_text('replacement')
        for name in ['browser.html', 'assets/browser.js', 'assets/locales.js', 'assets/browser.css']:
            with self.subTest(missing=name):
                resource = self.build / 'browser-ui' / name
                original = resource.read_bytes()
                resource.unlink()
                with self.assertRaisesRegex(RuntimeError, 'browser UI'):
                    installer.install(self.build, self.prefix)
                self.assertEqual(launcher.read_text(), 'fixture')
                self.assertEqual(desktop.read_bytes(), previous_desktop)
                installed = self.prefix / 'share/ccbaga-install/lib/CCBaga-A/browser-ui' / name
                self.assertEqual(installed.read_bytes(), original)
                resource.write_bytes(original)

    def test_failed_desktop_write_rolls_back_application(self):
        installer.install(self.build, self.prefix)
        (self.build / 'requirement-french').write_text('new')
        original = Path.write_text
        def fail_once(path, *args, **kwargs):
            if path.name == 'ccbaga-browser.desktop':
                raise OSError('fixture failure')
            return original(path, *args, **kwargs)
        with patch.object(Path, 'write_text', fail_once):
            with self.assertRaises(OSError):
                installer.install(self.build, self.prefix)
        self.assertEqual((self.prefix / 'bin/ccbaga-a').read_text(), 'fixture')

    def test_failed_install_and_rollback_retain_previous_application(self):
        installer.install(self.build, self.prefix)
        app = self.prefix / 'share/ccbaga-install'
        (self.build / 'requirement-french').write_text('replacement')
        original_write, original_rename = Path.write_text, Path.rename
        def fail_desktop(path, *args, **kwargs):
            if path.name == 'ccbaga-browser.desktop':
                raise OSError('fixture install failure')
            return original_write(path, *args, **kwargs)
        def fail_restore(path, destination):
            if path.name == 'previous' and path.parent.name.startswith('.ccbaga-stage-'):
                raise OSError('fixture rollback failure')
            return original_rename(path, destination)
        with patch.object(Path, 'write_text', fail_desktop), patch.object(Path, 'rename', fail_restore):
            with self.assertRaises((OSError, RuntimeError)) as caught:
                installer.install(self.build, self.prefix)
        backups = list(app.parent.glob('.ccbaga-stage-*/previous'))
        self.assertEqual(len(backups), 1)
        self.assertEqual((backups[0] / 'bin/requirement-french').read_text(), 'fixture')
        self.assertIn(str(backups[0].parent), str(caught.exception))

    def test_interrupted_directory_moves_restore_the_existing_installation(self):
        installer.install(self.build, self.prefix)
        (self.build / 'requirement-french').write_text('replacement')
        original = Path.rename
        app = self.prefix / 'share/ccbaga-install'
        for phase in ['old-before', 'old-after', 'new-before', 'new-after']:
            with self.subTest(phase=phase):
                def interrupt(path, destination):
                    selected = path == app if phase.startswith('old-') else path.name == 'new'
                    if selected and phase.endswith('before'):
                        raise KeyboardInterrupt('fixture cancellation')
                    result = original(path, destination)
                    if selected:
                        raise KeyboardInterrupt('fixture cancellation after successful rename')
                    return result
                with patch.object(Path, 'rename', interrupt), self.assertRaises(KeyboardInterrupt):
                    installer.install(self.build, self.prefix)
                self.assertEqual((app / 'bin/requirement-french').read_text(), 'fixture')
                self.assertFalse(list(app.parent.glob('.ccbaga-stage-*')))

    def test_cancelled_install_and_failed_restore_retain_backup(self):
        installer.install(self.build, self.prefix)
        original = Path.rename
        def interrupt_and_fail_restore(path, destination):
            if path.name == 'new':
                raise KeyboardInterrupt('fixture cancellation')
            if path.name == 'previous':
                raise OSError('fixture rollback failure')
            return original(path, destination)
        with patch.object(Path, 'rename', interrupt_and_fail_restore), self.assertRaisesRegex(RuntimeError, 'recovery stage'):
            installer.install(self.build, self.prefix)
        backups = list((self.prefix / 'share').glob('.ccbaga-stage-*/previous/bin/requirement-french'))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_text(), 'fixture')

    def test_exec_escaping_rejects_newlines_and_escapes_field_codes(self):
        self.assertIn('%%', installer.desktop_exec(Path('/tmp/100%/app')))
        self.assertIn('\\\\$', installer.desktop_exec(Path('/tmp/$secret/app')))
        with self.assertRaises(ValueError):
            installer.desktop_exec(Path('/tmp/app\nmalformed'))

    def test_exec_quoting_round_trips_through_both_desktop_entry_layers(self):
        def decode(value):
            # 先解码 Desktop Entry 字符串，再按 Exec 双引号规则还原参数。
            value = value.replace('\\\\', '\0')
            assert '\\' not in value
            value = value.replace('\0', '\\')
            assert value[0] == value[-1] == '"'
            out, i = '', 1
            while i < len(value) - 1:
                if value[i] == '\\':
                    i += 1
                else:
                    self.assertNotIn(value[i], '"`$')
                out += value[i]
                i += 1
            return out
        for text in ['/tmp/say "hi"/app', '/tmp/back\\slash/app', '/tmp/`tick`/$HOME/app',
                     '/tmp/mix "\\`$/app', '/tmp/中文 100% %U/app']:
            command = installer.desktop_exec(Path(text))
            self.assertTrue(command.startswith('/usr/bin/nice -n 0 -- '))
            self.assertEqual(decode(command.removeprefix('/usr/bin/nice -n 0 -- ').replace('%%', '%')), text)

    @unittest.skipUnless(sys.platform == 'linux', 'Real GIO parsing requires Linux; pure two-layer decoding still runs')
    def test_exec_round_trips_through_real_gio_with_distinct_url_arguments(self):
        gio = shutil.which('gio')
        self.assertIsNotNone(gio, 'Linux regression requires the GIO tool supplied with GLib')
        for index, name in enumerate(['say "hi"\\`tick`$HOME $(touch should-not-exist) 100% %U 中文',
                                      'CCBAGA_ASSIGNMENT=value % 中文', '--leading-option %U']):
            with self.subTest(name=name):
                self.check_gio_exec(gio, index, name)

    def check_gio_exec(self, gio, index, name):
        executable = self.root / name / 'app'
        executable.parent.mkdir()
        executable.write_text('#!/bin/sh\n/usr/bin/nice > "$CCBAGA_TEST_NICE"\nprintf \'%s\\0\' "$0" "$@" > "$CCBAGA_TEST_ARGV"\n')
        executable.chmod(0o700)
        receipt = self.root / f'argv-{index}'
        niceness = self.root / f'nice-{index}'
        desktop = self.root / 'fixture.desktop'
        desktop.write_text('[Desktop Entry]\nType=Application\nName=CCBaga fixture\n'
                           f'Exec={installer.desktop_exec(executable)} --browser %U\nTerminal=false\n')
        urls = ['https://example.invalid/first?q=two%20words&mark=%25',
                'https://example.invalid/second?q=%22quoted%22']
        # 直接启动临时桌面文件，不注册默认程序；子进程无凭据且 HOME/XDG 全部隔离。
        environment = {'PATH': os.defpath, 'HOME': str(self.root), 'XDG_DATA_HOME': str(self.root / 'data'),
                       'XDG_CONFIG_HOME': str(self.root / 'config'), 'XDG_CACHE_HOME': str(self.root / 'cache'),
                       'GIO_USE_VFS': 'local', 'GSETTINGS_BACKEND': 'memory', 'CCBAGA_TEST_ARGV': str(receipt),
                       'CCBAGA_TEST_NICE': str(niceness)}
        result = subprocess.run(['/usr/bin/nice', '-n', '7', '--', gio, 'launch', str(desktop), *urls], env=environment, cwd=self.root,
                                stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        expected = [str(executable).encode(), b'--browser', *(url.encode() for url in urls), b'']
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if receipt.exists() and receipt.read_bytes().split(b'\0') == expected:
                break
            time.sleep(.01)
        self.assertTrue(receipt.exists(), 'GIO did not launch the temporary fixture')
        self.assertEqual(receipt.read_bytes().split(b'\0'), expected)
        self.assertEqual(niceness.read_text().strip(), '7')
        self.assertFalse((self.root / 'should-not-exist').exists())

    def test_running_companion_blocks_replacement(self):
        installer.install(self.build, self.prefix)
        proc = self.root / 'proc-exe'
        proc.symlink_to(self.prefix / 'share/ccbaga-install/lib/CCBaga-A/browser-runtime/LinuxBrowser/chrome')
        original = Path.glob
        def processes(path, pattern):
            return iter([proc]) if path == Path('/proc') else original(path, pattern)
        (self.build / 'requirement-french').write_text('replacement')
        with patch.object(Path, 'glob', processes):
            with self.assertRaisesRegex(RuntimeError, 'RioBaga-B'):
                installer.install(self.build, self.prefix)
        self.assertEqual((self.prefix / 'bin/ccbaga-a').read_text(), 'fixture')


class PayloadTests(unittest.TestCase):
    def test_glibc_baseline_checks_actual_elf_and_rejects_newer_symbols(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / 'host'
            binary.write_bytes(b'\x7fELFfixture')
            with patch.object(payload.subprocess, 'check_output', return_value='Name: GLIBC_2.17 Name: GLIBC_2.35') as readelf:
                payload.verify_glibc([binary])
                self.assertEqual(readelf.call_args.args[0][-1], str(binary))
            with patch.object(payload.subprocess, 'check_output', return_value='Name: GLIBC_2.38'):
                with self.assertRaisesRegex(RuntimeError, 'glibc 2.35'):
                    payload.verify_glibc([binary])
            binary.write_text('not an ELF')
            with patch.object(payload.subprocess, 'check_output') as readelf:
                payload.verify_glibc([binary])
                readelf.assert_not_called()

    def test_payload_rejects_missing_ui_and_modified_browser(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            elf = b'\x7fELF\x02' + b'\0' * 13 + b'\x3e\x00'
            for name, content in {
                'usr/bin/requirement-french': elf,
                'usr/lib/CCBaga-A/browser-runtime/LinuxBrowser/chrome': elf,
                'usr/lib/CCBaga-A/browser-runtime/LICENSE.fingerprint-chromium': b'license',
                'usr/lib/CCBaga-A/browser-ui/browser.html': b'<script type="module" src="./assets/browser.js"></script>',
                'usr/lib/CCBaga-A/browser-ui/assets/browser.js': b'js',
                'usr/share/applications/ccbaga-browser.desktop': b'Exec=requirement-french --browser %U',
                'usr/share/icons/hicolor/512x512/apps/ccbaga-browser.png': b'icon',
                'source-engine/chrome': elf,
            }.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(content)
                path.chmod(0o755)
            payload.verify(root, root / 'source-engine/chrome')
            browser = root / 'usr/lib/CCBaga-A/browser-runtime/LinuxBrowser/chrome'
            isolated = root / 'usr/libexec/CCBaga-B/chrome'
            isolated.parent.mkdir(parents=True)
            browser.rename(isolated)
            payload.verify(root, root / 'source-engine/chrome')
            page = root / 'usr/lib/CCBaga-A/browser-ui/browser.html'
            page.unlink()
            with self.assertRaisesRegex(RuntimeError, 'UI'):
                payload.verify(root, root / 'source-engine/chrome')
            page.write_text('<script type="module" src="./assets/browser.js"></script>')
            library = root / 'source-engine/libfixture.so'
            library.write_text('original optional library')
            (isolated.parent / library.name).write_text('modified library')
            with self.assertRaisesRegex(RuntimeError, 'engine file'):
                payload.verify(root, root / 'source-engine/chrome')
            library.unlink()
            isolated.write_text('modified')
            with self.assertRaisesRegex(RuntimeError, 'pinned browser'):
                payload.verify(root, root / 'source-engine/chrome')

    def test_browser_ui_rejects_external_and_missing_actual_references(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'assets').mkdir()
            (root / 'assets/browser.js').write_text('fixture')
            page = root / 'browser.html'
            for source in ['https://example.invalid/upload.js', '../outside.js', '/outside.js',
                           'assets/missing.js', '%2e%2e/outside.js']:
                with self.subTest(source=source), self.assertRaisesRegex(RuntimeError, 'browser UI'):
                    page.write_text(f'<script type="module" src="{source}"></script>')
                    payload.verify_browser_ui(root)
            outside = root.parent / (root.name + '-outside.js')
            outside.write_text('outside fixture')
            try:
                (root / 'assets/external.js').symlink_to(outside)
                page.write_text('<script type="module" src="assets/external.js"></script>')
                with self.assertRaisesRegex(RuntimeError, 'browser UI'):
                    payload.verify_browser_ui(root)
            finally:
                outside.unlink()


if __name__ == '__main__':
    unittest.main()
