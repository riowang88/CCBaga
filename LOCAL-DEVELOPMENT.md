# 本地维护基线

此仓库用于维护 `riowang88/CCBaga` 的本地修改，源自 [liseami/CCBaga](https://github.com/liseami/CCBaga)。保留上游 Git 历史、MIT 许可和第三方声明。

- 日常工作目录：`~/Documents/CCBaga`。
- `origin`：`https://github.com/riowang88/CCBaga.git`（公开）；`upstream`：原作者仓库。日常修改推送 `origin/main`。
- 导入基线：上游 `5659b3c9fb7cf836e71707a77c8be267de393747` 加本地已验证修复，对应已安装的 `0.2.49+local.identity.6`。
- 源码版本基数已调整为 `0.2.50`；发布工作流按该基数加运行号递增，确保新版本高于已安装版本。上述本机版本通过构建时配置覆盖。

## 当前状态

身份保存、切换与浏览器资料联动修复已纳入源码。迁移前的 298 个源码文件与冻结清单逐一匹配。既有验证记录：71 项原生身份回归通过、3 项隔离真实 Chromium 会话测试通过（22 次探针）、4 个 UI 场景通过、4 项模型/文案测试通过（4829 条断言）。这些是已完成验证的记录，本次仓库迁移没有重新运行产品测试。

已安装应用二进制 SHA-256：`87a8a96d25b8fe1c5e42849fc4603fa3c1697b9139274b963e64d6f4d5b5871d`。源码冻结清单 SHA-256：`4e65d018e188efd4285166198b890bc98a925cbb9edf8e7a490817c4ef2b3415`。

本机 `releases/save-integrity-fix/` 保留安装包、冻结源码、测试日志与 `install-result.json`，该目录不进入 Git。构建阶段报告中的“未安装”是当时状态，最终安装状态以 `install-result.json` 为准。原工作目录保留用于回溯；后续开发使用本目录。

真实 macOS Keychain 写入/恢复、第三方服务端登录有效性和 Windows/Linux 实际运行不在上述验证范围内。本机安装使用 ad-hoc 签名，未作为官方版本公证发布。

## 后续维护

先按 README 的开发说明安装锁定依赖；迁移不复制 `node_modules` 与 Rust `target` 构建缓存。修改后运行与行为匹配的验证，再提交并推送 `origin/main`。本次导入提交使用 `[skip ci]`，后续普通提交仍沿用现有检查工作流。

身份保险库、浏览器资料、Cookie、密钥和个人配置属于应用运行数据，不放入源码仓库。`releases/`、身份归档和密钥沿用现有 Git 忽略规则。

应用内更新地址已配置为 `https://github.com/riowang88/CCBaga/releases/latest/download/latest.json`，使用本仓库独立公钥，仍强制校验签名与版本。macOS/Windows 更新下载地址和发布权限同步到本仓库，仅 `riowang88` 可触发及重跑发布；原有跨平台验收、公证和正式签名要求保留。Linux 仍通过安装包或包管理器更新。

独立更新密钥位于本机仓库外的 `~/.config/ccbaga-release/`（目录 0700、加密私钥与密码文件 0600），已配置 GitHub Secrets `TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。私钥不进入应用或 Git；维护者应妥善备份该目录。

截至更新渠道迁移时，本仓库没有 Release。正式发布仍缺六项 Apple Secrets：`APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_API_PRIVATE_KEY`、`APPLE_API_KEY`、`APPLE_API_ISSUER`、`CC_SIGN_IDENTITY`。取得有效凭据、完成发布验收后，才能生成可用的 `latest.json` 与签名更新包。没有 Release 时新地址返回错误，不能宣称“已是最新版本”。

当前安装的 `0.2.49+local.identity.6` 内嵌的仍是上游地址及公钥。源码配置不会修改已安装应用；首次迁移需安装包含新地址和公钥的构建，之后才能使用本仓库的更新。此次未构建、安装或发布新应用。

更新渠道验证：发布装配 3 项测试通过（含新地址接受、macOS/Windows 旧地址拒绝、未验收产物拒绝）；Tauri 官方更新器 2 项测试通过，使用新密钥签署的一次性测试载荷验证正常签名、字节篡改及版本冒用，不安装任何应用。发布所有者限制、版本基数与应用包标识另已核对。首次发布前仍需验证真实安装包、公证结果与线上下载。


## 0.2.50 本地源码快照（2026-10-28）

按用户“先这样，更新版本，提交git”冻结当前CLI/App分端登录、单端备份、进程生命周期核验与界面修订。package.json、Tauri配置、Cargo清单及锁文件同步为0.2.50。本次为本地提交，不等于公开Release、安装版更新或D1/D2全范围验收完成。

本轮提交检查：前端构建通过，110项前端测试通过；Rust custom-protocol测试172通过/5忽略/0失败；原生进程隔离及临时Keychain身份检查通过；7组identity-regressions通过。完整test-identities-ui.ts在第50行失败：要求Identity 1名称不存在，而基线HEAD和当前实现都优先保留当前档案名称。该套件因此未完成后续场景，不标为通过；本轮未改业务或放宽断言换取通过。证据在/tmp/ccbaga-0.2.50-check/。Windows/Linux和正式签名、公证、线上更新未重验；最新官方授权地址修复仅通过隔离验证，真实授权未再次执行。

本地候选已构建并启动，界面显示v0.2.50；路径/tmp/ccbaga-0.2.50-check/CCBaga-A.app，/Applications安装版未覆盖。
