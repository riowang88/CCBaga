<!--
[INPUT]: 英文 README 中的产品功能、支持平台、安装入口与使用边界。
[OUTPUT]: 简明中文项目入口、安装与操作导航，以及返回英文详细文档的链接。
[POS]: README.md 的中文入口；详细技术说明由现有文档维护。
[PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
-->

# RioBaga

[English](README.md) · **简体中文**

RioBaga 是面向旅居人员的桌面工具，包含两个应用：**RioBaga-A** 管理 Claude 身份与电脑区域设置，**RioBaga-B** 是使用独立资料目录的指纹 Chromium 浏览器。B 的实测结果经本机会话回到 A，设置与核验是两个独立步骤。

**用途声明：**本项目仅供旅居海外人员管理本人的 Claude 身份；禁止违反 Anthropic 官方用户协议的用途。本地设置不会更改服务端记录，也不能保证匿名或账号可用。

本页提供简明中文导航；完整功能、开发步骤和平台限制见[英文 README](README.md)。应用界面和侧栏的「使用说明」均支持中文，可在侧栏底部切换语言。

## 安装

从 [GitHub Releases](https://github.com/riowang88/RioBaga/releases/latest) 下载安装包。**每个安装包都同时包含 A 和 B**，没有单独的浏览器下载。

| 平台 | 安装包 |
| --- | --- |
| macOS（Apple Silicon，12+） | `RioBaga-<version>-macos-arm64.dmg` |
| Windows 10/11 x86_64 | `RioBaga-<version>-windows-x86_64-setup.exe` |
| Linux x86_64 | `RioBaga-<version>-linux-x86_64.deb` / `.rpm` / `.AppImage` / `.pkg.tar.zst`（Arch/pacman） |

- **macOS：**应用使用临时（ad-hoc）签名，**未经 Apple 公证**，首次打开可能被系统拦截；核实来源后可在「系统设置 → 隐私与安全性」中放行，组织策略也可能禁止打开。将 A、B 两个应用都拖入 Applications，然后启动 RioBaga-A。详见 [macOS 安装说明（英文）](README.md#macos)。
- **Windows：**安装器未做 Authenticode 签名，可能出现 SmartScreen 提示或被组织策略阻止。详见 [Windows 说明](docs/windows.md)。
- **Linux：**DEB、RPM、AppImage 基于 Ubuntu 22.04（glibc 2.35）构建，pacman 包面向当前 Arch。依赖与限制见 [Linux 说明](docs/linux.md)。

## 常用操作

- **电脑环境（默认页）：**选择国家或地区后一键设置时区、语言与区域格式，可选同时关闭定位、同时更换 RioBaga-B 浏览器指纹；也可逐项修改。地区设置不会改变公网 IP，部分操作需要系统授权、重启或注销。
- **Claude 身份管理：**保存备份或切换本地身份，卡片下方的保留项选择切换时保持哪些工作区资料；「更多工具」含分端登录与更新、桌面会话共享（macOS）、文件残留与 CLI 管理；「复制诊断日志」只复制本机有界日志，不上传；保存备份不改变当前工作区或浏览器绑定。可用密码加密包在同系统设备间导出/导入保存身份（导出跳过本机符号链接，导入先检测重复账号/指纹），不迁移网页登录。切换前保存工作；直接删除不可撤销且不会新增备份。
- **浏览器环境：**配置 RioBaga-B 的指纹种子、代理、语言时区、DNS 与默认浏览器；仅清缓存保留登录；全清理永久删除当前身份的密码、书签、历史、扩展、设置和站点数据，无备份，需单独确认。换指纹会重启 RioBaga-B。
- **网络环境：**纯只读检测页：出口 IP、Claude 四域名可用性、环境一致性、DNS 与 WebRTC/UDP 泄漏（后两项由 RioBaga-B 实测，WebRTC 仅在你点击检测时才连接固定公共 STUN）；未知或未检测不等于通过。应用不收集 SSH 密钥或代理凭据，也不执行服务器部署。

- **使用说明：**侧栏最后一页，按页给出编号操作说明、本软件用途声明与账号受限时的申诉步骤，支持中英文。

## 详细文档

- [使用速查（英文）](docs/usage.md)与[身份存储和恢复边界](docs/identity-storage.md)
- [更新说明（英文）](README.md#updates)
- [开发与检查（英文）](README.md#development)
- [贡献指南（英文）](CONTRIBUTING.md)

## 许可

RioBaga 基于 [liseami/CCBaga](https://github.com/liseami/CCBaga) 二次开发，原项目代码按 MIT 许可。RioBaga 同样以 [MIT 许可证](LICENSE)发布，第三方代码与二进制保留各自条款，见[第三方声明](NOTICE.md)。
