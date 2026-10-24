// Local-only integration: real native identity switches + persistent Chromium
// profiles + HttpOnly fixture login cookies. Never opens user profiles or Claude.
import {strict as assert} from 'node:assert';
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';
import {resolve, dirname, sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright-core';

const script = fileURLToPath(import.meta.url);
const repo = resolve(dirname(script), '..');
if (process.argv[2] === 'probe') {
  const [profile, account, mode] = process.argv.slice(3);
  assert.ok(profile.includes(`${sep}baga-opt-`) && profile.endsWith(`${sep}profile`));
  assert.ok(['A', 'B'].includes(account));
  const origin = process.env.CCBAGA_COOKIE_ORIGIN;
  assert.match(origin, /^http:\/\/127\.0\.0\.1:\d+$/);
  const context = await chromium.launchPersistentContext(profile, {
    executablePath: process.env.CCBAGA_TEST_ENGINE,
    headless: true,
    env: {...process.env, HOME: dirname(profile), XDG_CONFIG_HOME: dirname(profile), XDG_CACHE_HOME: dirname(profile)},
    args: ['--use-mock-keychain', '--password-store=basic', '--disable-background-networking'],
    serviceWorkers: 'block',
  });
  try {
    await context.route('**/*', route => route.request().url().startsWith(origin + '/') ? route.continue() : route.abort());
    const page = await context.newPage();
    if (mode === 'login') await page.goto(`${origin}/login/${account}`);
    const response = await page.goto(`${origin}/me`);
    assert.equal(response.status(), 200);
    assert.equal(await response.text(), `Authenticated fixture ${account}`);
    assert.equal(await page.evaluate(() => document.cookie.includes('fixture_session')), false, 'session must remain HttpOnly');
    console.log(`PASS Chromium ${mode} ${account}: persistent HttpOnly login survived profile reopening`);
  } finally {
    await context.close();
  }
} else {
  assert.ok(process.env.CCBAGA_TEST_ENGINE, 'set CCBAGA_TEST_ENGINE to the local Chromium executable');
  const server = createServer((req, res) => {
    res.setHeader('Cache-Control', 'no-store');
    const login = /^\/login\/([AB])$/.exec(req.url);
    if (login) {
      res.setHeader('Set-Cookie', `fixture_session=${login[1]}; Path=/; HttpOnly; SameSite=Lax; Max-Age=3600`);
      res.end('Local fixture login completed');
    } else if (req.url === '/me') {
      const session = /(?:^|;\s*)fixture_session=([AB])(?:;|$)/.exec(req.headers.cookie ?? '');
      res.statusCode = session ? 200 : 401;
      res.end(session ? `Authenticated fixture ${session[1]}` : 'Logged out');
    } else {
      res.statusCode = 404; res.end();
    }
  });
  await new Promise(done => server.listen(0, '127.0.0.1', done));
  try {
    const child = spawn(process.env.CCBAGA_TEST_CARGO ?? 'cargo', [
      'test', '--manifest-path', 'src-tauri/Cargo.toml',
      'identities::local_slot_tests::',
      '--', '--ignored', '--nocapture',
    ], {cwd: repo, stdio: 'inherit', env: {...process.env, CCBAGA_COOKIE_PROBE: script, CCBAGA_COOKIE_ORIGIN: `http://127.0.0.1:${server.address().port}`}});
    const code = await new Promise((done, reject) => {child.once('error', reject); child.once('exit', (code, signal) => done(signal ? 1 : code));});
    assert.equal(code, 0, 'native A→B→A with browser login probes must pass');
  } finally {
    server.closeAllConnections();
    await new Promise(done => server.close(done));
  }
}
