<!--
[INPUT]: 当前界面入口、身份保留策略和独立浏览器的清理范围。
[OUTPUT]: 双应用操作速查、显式浏览器全清理及本地数据边界。
[POS]: README 的操作指南；不执行命令或保存用户资料。
[PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
-->
# Usage guide

RioBaga is intended solely for people residing or traveling abroad to manage their own Claude identities. Use that violates Anthropic's official terms of service is prohibited.

Open **Guide** in the sidebar. The page supports English and Chinese.

## Quick reference

| Page | What to do |
| --- | --- |
| Easy setup | Choose a region and select an action. Clearing cache keeps sign-ins; changing the fingerprint restarts RioBaga-B. |
| Claude identities | Save your work before backing up or switching. Save backup retains the workspace and browser binding; Export/Import moves saved snapshots between devices with the same OS. Sign-in data follows the identity; choose what else to keep or archive. An empty identity starts a fresh local workspace. |
| Browser | Save settings, then open the check page. Clear cache retains sign-ins. Clear all browser data permanently removes the current identity's passwords, bookmarks, history, extensions, settings and sessions without a backup; confirm separately. |
| Computer | Adjust time zone, language and region separately. Permission, a restart or sign-out may be required. This does not change your public IP. |
| Network | Choose an AI tool and give it the copied prompt to install the Skill. The AI handles connection details and asks before remote deployment. The app does not collect SSH keys or perform deployment. |

Recognized accounts update the same saved identity instead of creating another copy. The current account appears once. **Link browser** is enabled by default and restores separate website data with each identity. The browser check page shows the local identity card above its environment checks. Save browser work before switching; first use requires website sign-in. Existing browser data is preserved; adopting or deleting it requires an explicit choice. See [local identity storage](identity-storage.md) for retention choices, archive size and recovery behavior.

**Save backup** confirms the exact archived categories and stops recognized Claude processes. Local expiry badges do not verify server access. **Export** requires a private password; **Import** adds a saved card without switching and excludes browser website sign-ins. See [storage and transfer limits](identity-storage.md#move-a-saved-identity-to-another-device).

Claude Code maintenance uninstalls or reinstalls the CLI program while retaining configuration, identities and project files.

## Data boundaries

- Identity operations use approved local directories. Actual project source files are not scanned.
- Permanent deletion cannot be undone. Review the confirmation before deleting current or archived data.
- Local changes do not erase Anthropic server records or guarantee anonymity or account access.
- A green result means the measured checks matched at that moment. DNS and WebRTC require separate checks.
- Keep passwords, private keys and tokens out of public logs and repositories.

Account use remains subject to Anthropic's terms and supported-region requirements. Changing local settings or a network route does not change those requirements.

## Restricted account

1. Save the message and related support tickets.
2. Sign in to claude.ai with the affected account and follow the appeal instructions.
3. Export data if the restricted page offers it. Do not bypass the restriction.

Available appeal and export options are determined by Anthropic. Consult the official pages for the applicable requirements:

- [Usage Policy](https://www.anthropic.com/legal/aup)
- [Consumer Terms](https://www.anthropic.com/legal/consumer-terms)
- [Supported countries and regions](https://www.anthropic.com/supported-countries)
- [Warnings and appeals](https://support.anthropic.com/en/articles/8241253-trust-and-safety-warnings-and-appeals)
