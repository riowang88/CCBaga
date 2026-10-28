#!/usr/bin/env python3
# [INPUT]: 各平台验收产物、完整覆盖目标版本的内部 SHA-256 清单与 macOS/Windows 更新签名。
# [OUTPUT]: 六个完整安装器、一个 macOS 更新归档、合并 latest.json 和总校验清单的明确公开文件列表。
# [POS]: 发布任务的白名单装配层；内部签名文件、引擎地图和公证记录留在 CI，不混入下载列表。
# [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
import hashlib
import json
import os
from pathlib import Path
import re

ROOT=Path(__file__).resolve().parent.parent
OUT=ROOT/'releases'

def main():
    version=os.environ['RELEASE_VERSION']
    if not re.fullmatch(r'\d+\.\d+\.\d+',version):raise SystemExit('Invalid release version')
    installers=[f'CCBaga-{version}-macos-arm64.dmg',f'CCBaga-{version}-windows-x86_64-setup.exe',*[f'CCBaga-{version}-linux-x86_64.{ext}' for ext in ['deb','rpm','AppImage','pkg.tar.zst']]]
    archive=f'CCBaga-{version}-macos-arm64.app.tar.gz'
    required={'SHA256SUMS':{installers[0],archive,archive+'.sig','latest.json'},
              'Linux-SHA256SUMS':set(installers[2:5]),'Arch-SHA256SUMS':{installers[5]},
              'Windows-SHA256SUMS':{installers[1]}}
    for name,artifacts in required.items():
        verified=set()
        for line in (OUT/name).read_text(encoding='utf-8').splitlines():
            digest,filename=line.split('  ',1)
            if Path(filename).name!=filename or not re.fullmatch(r'[0-9a-f]{64}',digest):raise SystemExit('Unsafe digest entry')
            with (OUT/filename).open('rb') as stream:
                if hashlib.file_digest(stream,'sha256').hexdigest()!=digest:raise SystemExit('Artifact digest mismatch')
            verified.add(filename)
        if not artifacts <= verified:raise SystemExit('Missing verified artifact in '+name)
    manifest=json.loads((OUT/'latest.json').read_text(encoding='utf-8'))
    windows=json.loads((OUT/'latest-windows.json').read_text(encoding='utf-8'))
    if manifest['version']!=version or windows['version']!=version:raise SystemExit('Mixed release versions')
    manifest['platforms'].update(windows['platforms'])
    expected={'darwin-aarch64':f'CCBaga-{version}-macos-arm64.app.tar.gz','windows-x86_64':f'CCBaga-{version}-windows-x86_64-setup.exe'}
    if set(manifest['platforms'])!=set(expected):raise SystemExit('Unexpected updater target')
    for platform,filename in expected.items():
        entry=manifest['platforms'][platform]
        url=f'https://github.com/riowang88/CCBaga/releases/download/v{version}/{filename}'
        if entry['url']!=url or entry['signature']!=(OUT/(filename+'.sig')).read_text(encoding='utf-8').strip():raise SystemExit('Updater URL/signature mismatch')
    manifest['notes']='macOS updates CCBaga-A; Windows updates both applications. Save unfinished browser work before updating.'
    (OUT/'latest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
    public=installers+[expected['darwin-aarch64'],'latest.json']
    digests=[]
    for filename in public:
        with (OUT/filename).open('rb') as stream:digests.append(f"{hashlib.file_digest(stream,'sha256').hexdigest()}  {filename}\n")
    (OUT/'SHA256SUMS').write_text(''.join(digests),encoding='utf-8')
    public.append('SHA256SUMS')
    (ROOT/'release-files.txt').write_text(''.join('releases/'+name+'\n' for name in public),encoding='utf-8')
    labels=['macOS Apple Silicon · A + B','Windows x64 · A + B','Linux DEB','Linux RPM','Linux AppImage','Arch/pacman']
    notes=['CCBaga combines recoverable local Claude identities, CLI management, real environment checks and an isolated companion browser.\n','\n| Platform | Installer |\n| --- | --- |\n']
    for label,filename in zip(labels,installers):notes.append(f'| {label} | [Download](https://github.com/riowang88/CCBaga/releases/download/v{version}/{filename}) |\n')
    notes+=['\nEach installer contains both applications. On macOS drag both apps into Applications. On Windows run the setup installer; SmartScreen may show a warning because the installer has no Authenticode certificate. Organization policy can restrict installation.\n','\nSidebar updates verify signed downloads: macOS updates A, Windows updates A and B. Linux uses its package manager. The archive and latest.json are used by automatic updates; ordinary installation only needs the platform installer. SHA256SUMS covers public downloads.\n','\n[Usage](https://github.com/riowang88/CCBaga#readme) · [Network Skill](https://github.com/riowang88/CCBaga/tree/main/skills/ccbaga-network)\n']
    if manifest.get('macos_signing') == 'adhoc':
        notes.append('\nmacOS: this build uses ad-hoc signing and is not notarized by Apple. First launch may be blocked. After verifying the source, review the app in System Settings > Privacy & Security. Organization policy may prohibit opening it. This does not disable or replace updater signature verification.\n')
    (ROOT/'release-notes.md').write_text(''.join(notes),encoding='utf-8')
    print('Nine public files assembled; internal verification material excluded.')

if __name__=='__main__':main()
