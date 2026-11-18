<!--
[INPUT]: 英文 README 中的产品功能、支持平台、安装入口与使用边界。
[OUTPUT]: 简明中文项目入口、安装与操作导航，以及返回英文详细文档的链接。
[POS]: README.md 的中文入口；详细技术说明由现有文档维护。
[PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
-->

> **本地维护仓库：**本仓库保存已安装的 `0.2.49+local.identity.6` 修复，后续维护见[维护基线](LOCAL-DEVELOPMENT.md)。下方下载链接和新构建的应用内更新已指向本仓库；首个 Release 尚需签名和公证凭据。现有安装版须先安装包含新更新地址与公钥的构建。

# RioBaga

[English](README.md) · **简体中文**

RioBaga 是面向 macOS、Linux 和 Windows 的本地工具，用于管理 Claude 身份、伴生浏览器指纹与系统环境，并核验实际浏览器环境。**RioBaga-A** 负责本机操作，**RioBaga-B** 是使用独立资料目录的伴生浏览器。

**用途声明：**本项目仅供旅居海外人员管理本人的 Claude 身份；禁止违反 Anthropic 官方用户协议的用途。本地设置不会更改服务端记录，也不能保证匿名或账号可用。

本页提供简明中文导航；完整功能、开发步骤和平台限制见[英文 README](README.md)。应用界面和侧栏的「使用说明」均支持中文，可在侧栏底部切换语言。

## 安装

从 [GitHub Releases](https://github.com/riowang88/CCBaga/releases/latest) 下载由本仓库维护者 **riowang88** 发布的安装包（首个版本尚未发布）。

- **macOS 12+，Apple Silicon：**下载包含 A、B 两个应用的 DMG，将两者拖入 Applications，然后启动 RioBaga-A。详见 [macOS 安装说明（英文）](README.md#macos)。
- **Linux x86_64：**提供基于 Ubuntu 22.04（glibc 2.35）的 DEB、RPM、AppImage，以及面向当前 Arch 的 pacman 包，均包含伴生浏览器。依赖与限制见 [Linux 说明](docs/linux.md)。
- **Windows 10/11 x86_64：**使用包含 A、B 的 NSIS 安装器。安装器未进行 Authenticode 签名，可能出现 SmartScreen 提示或被组织策略阻止。详见 [Windows 说明](docs/windows.md)。

## 常用操作

- **电脑环境（默认页）：**选择国家或地区后一键设置时区、语言与区域格式，可选同时关闭定位、同时更换 RioBaga-B 浏览器指纹；也可逐项修改。地区设置不会改变公网 IP，部分操作需要系统授权、重启或注销。
- **Claude 身份管理：**保存备份或切换本地身份，卡片下方的保留项选择切换时保持哪些工作区资料；「更多工具」含分端登录与更新、桌面会话共享（macOS）、文件残留与 CLI 管理；「复制诊断日志」只复制本机有界日志，不上传；保存备份不改变当前工作区或浏览器绑定。可用密码加密包在同系统设备间导出/导入保存身份（导出跳过本机符号链接，导入先检测重复账号/指纹），不迁移网页登录。切换前保存工作；直接删除不可撤销且不会新增备份。
- **浏览器环境：**配置 RioBaga-B 的指纹种子、代理、语言时区、DNS 与默认浏览器；仅清缓存保留登录；全清理永久删除当前身份的密码、书签、历史、扩展、设置和站点数据，无备份，需单独确认。换指纹会重启 RioBaga-B。
- **网络环境：**纯只读检测页：出口 IP、Claude 四域名可用性、环境一致性、DNS 与 WebRTC/UDP 泄漏（后两项由 RioBaga-B 实测，WebRTC 仅在你点击检测时才连接固定公共 STUN）；未知或未检测不等于通过。应用不再提供 Skill 入口或提示词，独立的 Network Skill 见仓库 skills/，由你自行让 AI 工具安装；应用不收集 SSH 密钥或执行服务器部署。

## 详细文档

- [使用速查（英文）](docs/usage.md)与[身份存储和恢复边界](docs/identity-storage.md)
- [更新说明（英文）](README.md#updates)
- [开发与检查（英文）](README.md#development)
- [贡献指南（英文）](CONTRIBUTING.md)
- [MIT 许可证](LICENSE)与[第三方声明](NOTICE.md)
