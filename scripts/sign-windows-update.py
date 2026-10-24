#!/usr/bin/env python3
# [INPUT]: 已验收 Windows 安装器、独立步骤注入的更新私钥与发布版本。
# [OUTPUT]: 绑定版本的安装器签名和内部 Windows 更新清单，并用官方更新器核验真实字节。
# [POS]: 不构建应用、不读取 Apple 凭据；仅签名子进程接触更新私钥，测试只接收公开清单。
# [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
import json
import os
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parent.parent
version=os.environ['CC_RELEASE_VERSION']
installer=ROOT/'releases'/f'CCBaga-{version}-windows-x86_64-setup.exe'
env={k:v for k,v in os.environ.items() if not k.startswith('APPLE_')}
result=subprocess.run(['bun','run','tauri','signer','sign','--app-version',version,str(installer)],cwd=ROOT,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
if result.returncode:raise SystemExit('Windows updater signing failed')
signature=Path(str(installer)+'.sig').read_text(encoding='utf-8').strip()
manifest=installer.parent/'latest-windows.json'
manifest.write_text(json.dumps({'version':version,'platforms':{'windows-x86_64':{'url':f'https://github.com/riowang88/CCBaga/releases/download/v{version}/{installer.name}','signature':signature}}},indent=2)+'\n',encoding='utf-8')
clean={k:v for k,v in env.items() if not k.startswith('TAURI_SIGNING_PRIVATE_KEY')}
clean.update(CC_UPDATE_MANIFEST=str(manifest),CC_UPDATE_ARCHIVE=str(installer))
subprocess.run(['cargo','test','--locked','--manifest-path','src-tauri/Cargo.toml','--features','custom-protocol','published_archive_matches_embedded_key_and_signed_version','--','--ignored'],cwd=ROOT,env=clean,check=True)
print('Windows installer updater signature and signed version verified.')
