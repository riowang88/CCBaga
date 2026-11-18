/**
 * [INPUT]: 共享 YAML 中英文目录与各页面的操作边界（按 2026-11 现行界面逐项核对：侧栏顺序、网络纯检测、身份更多工具四段与诊断日志、导出跳过本机链接、B 默认禁止未代理 UDP）。
 * [OUTPUT]: 旅居人员本人身份管理用途声明；按侧栏顺序的五项编号说明（电脑环境/网络环境/Claude 身份/浏览器环境/RioBaga-A 与 RioBaga-B，含删除与全清理不可撤销、不新增备份、WebRTC「未知」不等于通过、部署见仓库 skills/）；折叠的账号边界/申诉步骤。
 * [POS]: 只读帮助页；区分仅缓存与当前身份全部本地浏览器资料删除，奶油/珊瑚视觉（help.css：衬线标题、奶油声明卡、细线编号卡与奶油折叠卡）和原生 details，不调用原生命令或读取用户数据。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import { useI18n } from './i18n';
import './help.css';

export function HelpPanel() {
    const { t } = useI18n();
    const topics = [
        { title: t('电脑环境'), lines: [
            t('选一个地区，「一键按地区设置」一次改好时区、首选语言和区域格式；可勾选同时关闭定位或同时更换 B 浏览器指纹。'),
            t('下方「当前设置」可逐项修改时区和语言。改时区需要管理员密码，改语言后需重开应用；这些都不会改变公网 IP。')
        ] },
        { title: t('网络环境'), lines: [
            t('只做检测、不改设置：本机出口 IP、所在地与运营商，Claude 四个域名的可用性，以及浏览器与电脑的环境一致性。'),
            t('DNS 泄露和 WebRTC/UDP 泄露由 B 实际检测，A 只读取最近一轮结果；「在 B 中打开完整报告」需要先打开 B。'),
            t('B 默认禁止不走代理的 UDP，WebRTC 常常取不到结果而显示「未知」；「未知」不等于通过。')
        ] },
        { title: t('Claude 身份'), lines: [
            t('卡片依次是空身份、当前身份和已保存身份：主按钮保存/更新备份或切换，重命名、导出、删除在「···」菜单里。'),
            t('暂存和切换会结束 Claude 进程，请先保存任务。'),
            t('卡片下方「切换账号时保持不变」：白底实线的留在工作区，淡底虚线的跟身份一起存走；登录信息始终跟身份走。'),
            t('「更多工具」分四段：分端登录与更新、桌面会话共享、文件残留、CLI 管理。遇到问题可点「复制诊断日志」，日志只在本机、不上传。'),
            t('导出身份需设置迁移密码；本机符号链接不会导出，完成时提示跳过的数量。'),
            t('直接删除当前身份、删除暂存身份和清理文件残留都无法撤销，也不会新增备份。')
        ] },
        { title: t('浏览器环境'), lines: [
            t('只管 RioBaga-B 的配置和数据：指纹种子、代理、语言与时区、DNS、设备指纹。修改时先正常退出 B，保存后再由 A 打开。'),
            t('清理缓存保留登录、书签和密码。'),
            t('全清理会删除当前身份的密码、书签、历史、扩展和设置，不留备份。')
        ] },
        { title: t('RioBaga-A 与 RioBaga-B'), lines: [
            t('RioBaga-A 是本程序，负责身份管理、电脑设置与检测；RioBaga-B 是独立的指纹浏览器。'),
            t('代理与服务器部署不在本软件界面中；需要时参阅仓库 skills/ 目录的说明。')
        ] }
    ];
    return <section className="rb-page help-page">
        <header><h1 className="rb-h1">{t('使用说明')}</h1></header>
        <p className="rb-card help-scope">{t('本软件仅供旅居人员管理本人的 Claude 身份，禁止用于违反 Anthropic 官方用户协议的行为。')}</p>
        <ol className="rb-card line help-topics">{topics.map((topic, index) => <li className="help-row" key={index}>
            <div className="help-label"><span aria-hidden="true">{String(index + 1).padStart(2, '0')}</span><h2>{topic.title}</h2></div>
            <div className="help-copy">{topic.lines.map(line => <p key={line}>{line}</p>)}</div>
        </li>)}</ol>
        <div className="rb-card help-notes">
            <details className="help-details">
                <summary>{t('账号与数据')}<span aria-hidden="true"/></summary>
                <div className="help-copy">
                    <p>{t('本地操作不删除服务端记录，也不保证匿名或账号可用。')}</p>
                    <p>{t('绿色结果仅表示本次已测项目匹配；DNS 和 WebRTC 需另行检查。')}</p>
                    <p>{t('账号使用仍须遵守 Anthropic 的条款和支持地区要求。')}</p>
                    <p>{t('密码、密钥和令牌不要放进公开日志或仓库。')}</p>
                </div>
            </details>
            <details className="help-details">
                <summary>{t('账号受限怎么办')}<span aria-hidden="true"/></summary>
                <ol className="help-appeal">
                    <li>{t('保存页面提示和相关工单。')}</li>
                    <li>{t('用受影响的账号登录 claude.ai，按页面指引申诉。')}</li>
                    <li>{t('如页面提供导出入口，可下载数据；不要绕过限制。')}</li>
                </ol>
            </details>
        </div>
    </section>;
}
