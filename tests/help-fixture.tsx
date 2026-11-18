/**
 * [INPUT]: 真实 HelpPanel 与共享外壳；页面不调用原生命令，无需 mockIPC。
 * [OUTPUT]: 使用说明页可视核对样本；window.helpFixture.language 切换中英文。
 * [POS]: 仅测试/人工核对使用，不进入生产构建。
 * [PROTOCOL]: 变更时更新此头部，然后检查 CLAUDE.md
 */
import '../src/index.css';
import '../src/style.css';
import {createRoot} from 'react-dom/client';
import {HelpPanel} from '../src/help';
import {setLocale} from '../src/i18n';
import {FixtureShell} from './fixture-shell';
Object.assign(window,{helpFixture:{language:setLocale}});
setLocale('zh');
createRoot(document.getElementById('fixture')!).render(<FixtureShell page="help"><HelpPanel/></FixtureShell>);
