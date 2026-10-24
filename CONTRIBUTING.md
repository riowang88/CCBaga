<!--
[INPUT]: 仓库模块边界、测试入口与 GEB 契约。
[OUTPUT]: PR 作者需要遵守的修改、验证和隐私规则。
[POS]: README 的贡献入口，不提供发布操作流程。
[PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
-->
# Contributing to CCBaga

Follow the [development guide](README.md#development) or [Linux setup](docs/linux.md). Use the existing lockfiles and UI components.

## Pull requests

- Describe the problem, resulting behavior, checks performed and remaining limits.
- Read the relevant `CLAUDE.md` map. Update file contracts and module maps with your changes.
- Keep native operations in Rust/platform bridges and UI strings in both YAML catalogs.
- Preserve cleanup scopes, process identity checks, permissions and secret handling. Add a focused regression when these boundaries change.
- Run the relevant [checks](README.md#checks); verify documentation links for documentation changes.
- Preserve third-party notices. Original contributions use the [MIT License](LICENSE).

Releases from this repository are published exclusively by **riowang88**. Upstream releases remain maintained by **liseami**. Contributor builds are for development and testing.

## Bug reports

Include the operating system, app version, operation, expected behavior and sanitized error. Never attach private keys, tokens, passwords, subscription URLs or personal file contents. Report sensitive security details privately to the project owner.
