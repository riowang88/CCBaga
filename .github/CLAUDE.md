# .github/
> L2 | 父级: ../CLAUDE.md
workflows/ - release 分支 push 的更新签名、可选 Apple 公证与双应用发布，以及 PR/main 的 Ubuntu/macOS/Windows 只读检查；见其 CLAUDE.md。
dependabot.yml: 每周跟进 Actions 的固定提交与依赖更新，更新通过 PR 审阅。
[PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md

原生身份钥匙串夹具在 macOS 编译后显式临时签名，以验证现代钥匙串签名分区；仅修改临时测试可执行文件，不访问发布签名凭据。
