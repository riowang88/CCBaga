# [INPUT]: 发布阶段的双应用暂存目录、压缩像素背景与清理器 ICNS。
# [OUTPUT]: dmgbuild 的 Finder 窗口、背景、拖拽位置与隐藏通知文件配置。
# [POS]: 仅定义安装盘布局；签名、公证和最终校验由 release-macos.py 执行。
# [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
from pathlib import Path

ROOT = Path(defines['root'])
stage = Path(defines['stage'])
files = [str(stage / name) for name in ['RioBaga-A.app', 'RioBaga-B.app', 'LICENSE', 'NOTICE.md', 'Install.txt']]
symlinks = {'Applications': '/Applications'}
hide = ['LICENSE', 'NOTICE.md', 'Install.txt']
icon = str(ROOT / 'src-tauri/icons/icon.icns')
background = str(ROOT / 'assets/dmg-background.png')
format = 'UDZO'
window_rect = ((100, 100), (800, 500))
default_view = 'icon-view'
include_icon_view_settings = True
include_list_view_settings = False
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
arrange_by = None
text_size = 16
icon_size = 112
icon_locations = {
    'RioBaga-A.app': (200, 200),
    'RioBaga-B.app': (200, 350),
    'Applications': (600, 275),
}
