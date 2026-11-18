#!/usr/bin/env python3
# [INPUT]: NSIS 安装器、固定引擎哈希、browser_ui 资源验收、Skill Windows 私有文件权限层与空白 GitHub Windows runner。
# [OUTPUT]: 当前用户私密样本、UTF-8 诊断与安装/实际 A/B 回环/升级/卸载/资产保留验收；仅沙箱缓存拒绝清理交由一次性 runner 销毁。
# [POS]: 只在可丢弃 CI runner 执行安装；拒绝已有应用资料，不读取开发者用户数据。
# [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
import ctypes
from ctypes import wintypes
import uuid
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request
from browser_ui import verify_browser_ui

ROOT=Path(__file__).resolve().parent.parent

# ----- Windows 重定向输出也使用 UTF-8，中文启动错误不得遮住原始失败 -----
for stream in (sys.stdout, sys.stderr):
    if hasattr(stream, 'reconfigure'):
        stream.reconfigure(encoding='utf-8', errors='backslashreplace')

def wait_for(check, seconds=45):
    deadline=time.monotonic()+seconds
    while time.monotonic()<deadline:
        if check():return
        time.sleep(.25)
    raise AssertionError('Windows runtime verification timed out')

def main():
    if os.name!='nt' or os.environ.get('GITHUB_ACTIONS')!='true':
        raise SystemExit('Installer mutation is restricted to a disposable GitHub Windows runner')
    trace=Path(tempfile.gettempdir())/'ccbaga-ci-startup.log';trace.unlink(missing_ok=True)
    installer=Path(sys.argv[1]).resolve()
    manifest=json.loads(Path(sys.argv[2]).read_text(encoding='utf-8'))
    # ----- 与 Tauri dirs::data_dir 相同的 Known Folder，宿主和验收进程保持相同环境 -----
    shell=ctypes.WinDLL('shell32');ole=ctypes.WinDLL('ole32')
    known=shell.SHGetKnownFolderPath
    known.argtypes=[ctypes.c_void_p,wintypes.DWORD,wintypes.HANDLE,ctypes.POINTER(ctypes.c_void_p)];known.restype=ctypes.c_long
    ole.CoTaskMemFree.argtypes=[ctypes.c_void_p]
    folder_id=ctypes.create_string_buffer(uuid.UUID('3EB685DB-65F9-4CF6-A03A-E3EF65729F3D').bytes_le)
    address=ctypes.c_void_p()
    assert known(folder_id,0,None,ctypes.byref(address))==0
    try:data=Path(ctypes.wstring_at(address))/'ai.local.requirement-french'
    finally:ole.CoTaskMemFree(address)
    if data.exists():raise SystemExit('Refusing to touch pre-existing CCBaga application data')
    with tempfile.TemporaryDirectory(prefix='ccbaga-installer-') as temporary:
        folder=(Path(temporary)/'应用 with spaces').resolve()
        home=Path(os.environ['USERPROFILE'])
        claude=home/'.claude'
        if claude.exists():raise SystemExit('Refusing to touch pre-existing Claude data on the CI runner')
        skill=home/'.claude/skills/personal/SKILL.md';skill.parent.mkdir(parents=True);skill.write_text('keep-skill',encoding='utf-8')
        settings=home/'.claude/settings.json';settings.write_text('{"keep":true}',encoding='utf-8')
        profile=data/'browser/profile/Default/personal-data';profile.parent.mkdir(parents=True);profile.write_bytes(b'keep-profile')
        # ----- 只修正刚创建的 CI 样本；管理员 token 的默认所有者不能冒充用户 SID -----
        sys.path.insert(0,str(ROOT/'skills/ccbaga-network/scripts'))
        import windows as private_files
        for sample in (data,claude):
            # 先保护子项，避免父目录移除继承 ACE 时暂时失去子项访问权。
            for path in sorted([sample,*sample.rglob('*')],key=lambda p:len(p.parts),reverse=True):
                private_files.protect(path)
        env={k:v for k,v in os.environ.items() if not k.startswith(('APPLE_','TAURI_SIGNING_PRIVATE_KEY'))}
        env.update(CC_CLI_USER_DIR=str(home),HTTPS_PROXY='http://secret-marker.invalid:1')
        def powershell(command,extra=None,quiet=True):
            script="$env:PSModulePath=$PSHOME+'\\Modules;'+$env:PSModulePath; $ErrorActionPreference='Stop'; "+command
            result=subprocess.run(['powershell','-NoProfile','-NonInteractive','-Command',script],env=env|(extra or {}),stdout=subprocess.DEVNULL if quiet else None,stderr=subprocess.PIPE)
            if result.stderr:raise AssertionError('CI PowerShell query failed: '+result.stderr.decode('utf-8',errors='replace')[-2048:])
            return result.returncode
        def install():
            subprocess.run(f'"{installer}" /S /D={folder}',check=True,timeout=180,env=env)
        def payload():
            assert (folder/'requirement-french.exe').is_file()
            engine=folder/'browser-runtime/WindowsBrowser'
            if not engine.is_dir():engine=folder/'WindowsBrowser'
            actual={p.relative_to(engine).as_posix():hashlib.file_digest(p.open('rb'),'sha256').hexdigest() for p in engine.rglob('*') if p.is_file()}
            assert actual==manifest,'Installed Chromium payload differs from the verified upstream engine'
            pages=list(folder.rglob('browser.html'))
            assert len(pages)==1,'Expected exactly one embedded browser page'
            verify_browser_ui(pages[0].parent)
            assert any(folder.rglob('browser.ico')),'Missing companion icon'
            return engine
        def assets():
            assert profile.read_bytes()==b'keep-profile'
            assert skill.read_text(encoding='utf-8')=='keep-skill'
            assert json.loads(settings.read_text(encoding='utf-8'))=={'keep':True}
        child=None
        runtime_log=Path(temporary)/'runtime.log'
        def stop():
            nonlocal child
            # GUI 引擎管理自己的沙箱 Job；不额外将宿主放入测试 Job。
            if child and child.poll() is None:
                subprocess.run(['taskkill','/F','/T','/PID',str(child.pid)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=False)
            if child:child.wait(timeout=20);child=None
            # 仅回收本次唯一安装目录内的 CI 程序，处理浏览器自己的独立生命周期。
            command="function N([string]$s){ $s=$s.Replace('/','\\'); if($s.StartsWith('\\\\?\\')){return $s.Substring(4)};return $s }; "+'$p=@(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and (N $_.ExecutablePath).StartsWith((N $env:CC_INSTALL_DIR)+"\\",[StringComparison]::OrdinalIgnoreCase) }); $p | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }; if($p.Count){exit 1}'
            wait_for(lambda:powershell(command,{'CC_INSTALL_DIR':str(folder)})==0)
        def ready():
            if child.poll() is not None:
                raise AssertionError(f'Installed host exited with status {child.returncode}')
            return start.is_file() and bool(start.read_text(encoding='utf-8').strip())
        try:
            install();engine=payload();assets()
            with runtime_log.open('wb') as output:
                child=subprocess.Popen([str(folder/'requirement-french.exe')],env=env,stdout=output,stderr=output)
            start=data/'browser/start.url'
            wait_for(ready)
            page=start.read_text(encoding='utf-8').strip()
            opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
            with opener.open(page,timeout=15) as response:assert response.status==200
            context=page.rsplit('/',1)[0]+'/context'
            with opener.open(context,timeout=15) as response:
                body=response.read(131072)
                assert b'secret-marker' not in body and b'system_proxy' not in body
                assert isinstance(json.loads(body),dict)
            machine=json.loads(body)['machine']
            def measured():
                with opener.open(page.rsplit('/',1)[0]+'/report',timeout=15) as response:observed=json.load(response)
                report=observed.get('report')
                if not report:return False
                assert observed['received_at']>0
                assert report['platform']=='Win32' and report['cores']==machine['cores']
                assert report['platform_version']==machine['version'],'Actual browser OS version differs from the native machine'
                assert report['browser_version']==(engine/'version.txt').read_text(encoding='utf-8').strip()
                return True
            wait_for(measured)
            assert child.poll() is None,'Installed host exited before verification'
            # ----- 实际同资料浏览器进程：仅查询 CI 样本路径，不打印命令行 -----
            query="function N([string]$s){ $s=$s.Replace('/','\\'); if($s.StartsWith('\\\\?\\')){return $s.Substring(4)};return $s }; "+'$p=@(Get-CimInstance Win32_Process -Filter "Name = \'chrome.exe\'" | Where-Object { (N $_.ExecutablePath) -like ((N $env:CC_INSTALL_DIR)+"*") -and (N $_.CommandLine) -like ("*"+(N $env:CC_PROFILE)+"*") }); if($p.Count -eq 0){exit 1}'
            wait_for(lambda:powershell(query,{'CC_INSTALL_DIR':str(folder),'CC_PROFILE':str(profile.parent.parent)})==0)
            stop()
            install();payload();assets()
            uninstallers=list(folder.glob('*uninstall*.exe'))
            assert len(uninstallers)==1,'Expected one application uninstaller'
            subprocess.run(f'"{uninstallers[0]}" /S _?={folder}',check=True,timeout=180,env=env)
            wait_for(lambda:not (folder/'requirement-french.exe').exists())
            assets()
            print('Windows current-user install, A/B launch, upgrade and uninstall verified; user assets preserved.')
        except Exception:
            print('CI startup diagnostic:',runtime_log.read_text(encoding='utf-8',errors='replace')[-8192:].replace('secret-marker','[test-marker]'))
            print('CI start file exists:',(data/'browser/start.url').exists())
            diagnostic='$p=@(Get-CimInstance Win32_Process -Filter "Name = \'chrome.exe\'"); @($p | ForEach-Object { [pscustomobject]@{image_available=[bool]$_.ExecutablePath;command_available=[bool]$_.CommandLine;image_match=[bool]($_.ExecutablePath -like ($env:CC_INSTALL_DIR+"*"));profile_match=[bool]($_.CommandLine -like ("*"+$env:CC_PROFILE+"*"))} }) | ConvertTo-Json -Compress'
            powershell(diagnostic,{'CC_INSTALL_DIR':str(folder),'CC_PROFILE':str(profile.parent.parent)},quiet=False)
            print('CI fixed startup stages:',trace.read_text(encoding='utf-8',errors='replace')[-4096:] if trace.exists() else 'no host entry')
            powershell('$p=Get-Process -Id $env:CC_TEST_PID -ErrorAction SilentlyContinue; $p | Select-Object ProcessName,Id,Responding,MainWindowTitle; $p.Modules | ForEach-Object { $_.ModuleName }',{'CC_TEST_PID':str(child.pid if child else 0)},quiet=False)
            raise
        finally:
            stop()
            try:
                shutil.rmtree(data)
            except PermissionError as error:
                # 只允许 Chromium 沙箱缓存留给一次性 runner 销毁；其他清理错误仍失败。
                cache=data/'browser/profile/Default/Cache'
                if not error.filename or not Path(error.filename).is_relative_to(cache):
                    raise
                print('CI sandbox cache cleanup deferred to disposable runner teardown.')
            shutil.rmtree(claude)
            trace.unlink(missing_ok=True)

if __name__=='__main__':main()
