/**
 * [INPUT]: 共享 YAML 中英文目录与各页面的操作边界。
 * [OUTPUT]: 旅居人员本人身份管理用途声明、五项编号说明与折叠的账号边界/申诉步骤。
 * [POS]: 只读帮助页；区分仅缓存与当前身份全部本地浏览器资料删除，奶油/珊瑚视觉（help.css：衬线标题、奶油声明卡、细线编号卡与奶油折叠卡）和原生 details，不调用原生命令或读取用户数据。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import { useI18n } from './i18n';
import './help.css';

export function HelpPanel() {
    const { t } = useI18n();
    const topics = [
        { title: t('傻瓜设置'), lines: [
            t('选择地区，再点需要的操作。'),
            t('缓存保留登录；换指纹会重启浏览器。')
        ] },
        { title: t('Claude 身份管理'), lines: [
            t('暂存和切换会结束 Claude 进程，请先保存任务。'),
            t('登录随身份保存，其他资料由你选择保留或暂存。'),
            t('空身份留出新的本地工作区；永久删除无法恢复。')
        ] },
        { title: t('浏览器环境'), lines: [
            t('先保存设置，再打开实测页。'),
            t('全清理会删除当前身份的密码、书签、历史、扩展和设置，不留备份。')
        ] },
        { title: t('电脑环境'), lines: [
            t('分别调整时区、语言和地区，不会改变公网 IP。'),
            t('系统可能要求授权、重启或重新登录。')
        ] },
        { title: t('网络与 Skill'), lines: [
            t('选择 AI 工具，复制提示词并交给它安装 Skill。'),
            t('连接资料交给 AI；远端部署前由你确认。')
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
