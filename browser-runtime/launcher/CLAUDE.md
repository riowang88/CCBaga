# launcher/
> L2 | 父级: ../CLAUDE.md
CCBrowser.c: macOS LaunchServices 入口；宿主按同目录 RioBaga-A.app → 旧名 CCBaga-A.app 查找且须 Bundle ID 一致，否则 open -b；等待本次宿主发布资料参数，身份事务期间阻断启动；参数缺失或恢复未完成时拒绝回退到其他资料，链接与首个实测页沿用宿主会话。
host_lookup.h: 宿主应用包纯查找（新名优先、旧名兼容、CoreFoundation 读取实际包标识），由 scripts/test-browser-launcher.py 用临时伪应用包编译测试。
[PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
