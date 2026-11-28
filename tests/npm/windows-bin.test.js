const assert = require('node:assert/strict');
const { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join, relative } = require('node:path');
const { test } = require('node:test');
const optimize = require('../../npm/lib/windows-bin');

test('Windows optimization changes only command shims targeting this installation', () => {
  const root = mkdtempSync(join(tmpdir(), 'ad-windows-bin-'));
  const bin = join(root, 'node_modules/agent-desktop/bin');
  mkdirSync(bin, { recursive: true });
  const binary = join(bin, 'agent-desktop-win32-x64.exe');
  writeFileSync(binary, 'native');
  const command = join(root, 'agent-desktop.cmd');
  const otherCommand = join(root, 'node_modules/.bin/agent-desktop.cmd');
  mkdirSync(join(root, 'node_modules/.bin'), { recursive: true });
  const unrelated = '@echo off\nnode other-package/cli.js %*\n';
  writeFileSync(otherCommand, unrelated);
  const target = relative(root, join(bin, 'agent-desktop'));
  writeFileSync(command, `@echo off\nnode "${target}" %*\n`);
  const powershell = join(root, 'agent-desktop.ps1');
  writeFileSync(powershell, `& node "$basedir/${target}" @args\n`);
  assert.equal(optimize(bin, 'agent-desktop-win32-x64.exe'), true);
  const installed = readFileSync(command, 'utf8');
  assert.ok(installed.includes(`"${binary}" %*`));
  assert.doesNotMatch(installed, /node(?:\.exe)?[ "\t]/);
  assert.equal(readFileSync(powershell, 'utf8'), `& '${binary.replace(/'/g, "''")}' @args\r\nexit $LASTEXITCODE\r\n`);
  assert.equal(readFileSync(otherCommand, 'utf8'), unrelated);
});

for (const prefix of ['Пользователь', '用户']) {
  test(`Windows optimization preserves npm shims for a ${prefix} prefix`, (t) => {
    const root = mkdtempSync(join(tmpdir(), 'ad-windows-bin-'));
    t.after(() => rmSync(root, { recursive: true, force: true }));
    const destination = join(root, prefix);
    const bin = join(destination, 'node_modules/agent-desktop/bin');
    mkdirSync(bin, { recursive: true });
    writeFileSync(join(bin, 'agent-desktop-win32-x64.exe'), 'native');
    const target = relative(destination, join(bin, 'agent-desktop'));
    const command = join(destination, 'agent-desktop.cmd');
    const powershell = join(destination, 'agent-desktop.ps1');
    const commandContents = Buffer.from(`@echo off\r\nnode "%~dp0/${target}" %*\r\n`);
    const powershellContents = Buffer.from(`& node "$basedir/${target}" @args\r\n`);
    writeFileSync(command, commandContents);
    writeFileSync(powershell, powershellContents);

    assert.equal(optimize(bin, 'agent-desktop-win32-x64.exe'), false);
    assert.deepEqual(readFileSync(command), commandContents);
    assert.deepEqual(readFileSync(powershell), powershellContents);
  });
}
