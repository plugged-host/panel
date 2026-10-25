// plugged.host: tests for the translations merge driver (`node --test merge-translations.test.mjs`).
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { main, mergeTranslations } from './merge-translations.mjs';

test("upstream's new and changed keys come in, ours stay", () => {
  const base = { a: 'A', b: 'B', nested: { x: 'X' } };
  const ours = { a: 'A', b: 'B (ours)', nested: { x: 'X', ours: 'O' } };
  const theirs = { a: 'A (theirs)', b: 'B', nested: { x: 'X', theirs: 'T' }, added: 'N' };

  const { merged, kept } = mergeTranslations(base, ours, theirs);

  assert.deepEqual(merged, {
    a: 'A (theirs)',
    b: 'B (ours)',
    nested: { x: 'X', ours: 'O', theirs: 'T' },
    added: 'N',
  });
  assert.deepEqual(kept, []);
});

test('a key changed differently on both sides keeps ours and is listed', () => {
  const { merged, kept } = mergeTranslations({ a: { b: 'old' } }, { a: { b: 'ours' } }, { a: { b: 'theirs' } });

  assert.deepEqual(merged, { a: { b: 'ours' } });
  assert.deepEqual(kept, ['a.b']);
});

test('the same change on both sides is no conflict', () => {
  const { merged, kept } = mergeTranslations({ a: 'old' }, { a: 'new' }, { a: 'new' });

  assert.deepEqual(merged, { a: 'new' });
  assert.deepEqual(kept, []);
});

test('a removal on one side wins over an unchanged key, not over our change', () => {
  const base = { gone: 'G', changed: 'C', theirsGone: 'T' };
  const ours = { changed: 'C (ours)', theirsGone: 'T' };
  const theirs = { gone: 'G' };

  const { merged, kept } = mergeTranslations(base, ours, theirs);

  assert.deepEqual(merged, { changed: 'C (ours)' });
  assert.deepEqual(kept, ['changed']);
});

test('keys keep their order, ours after the key they follow', () => {
  const base = { a: 'A', c: 'C' };
  const ours = { a: 'A', ourFirst: '1', c: 'C', ourLast: '2' };
  const theirs = { start: 'S', a: 'A', b: 'B', c: 'C' };

  const { merged } = mergeTranslations(base, ours, theirs);

  assert.deepEqual(Object.keys(merged), ['start', 'a', 'ourFirst', 'b', 'c', 'ourLast']);
});

test('a key of ours before any of theirs goes first', () => {
  const { merged } = mergeTranslations({}, { mine: 'M', a: 'A' }, { a: 'A', b: 'B' });

  assert.deepEqual(Object.keys(merged), ['mine', 'a', 'b']);
});

test('a file new on both sides merges without a base', () => {
  const { merged, kept } = mergeTranslations(undefined, { a: 'ours', only: 'O' }, { a: 'theirs', b: 'B' });

  assert.deepEqual(merged, { a: 'ours', b: 'B', only: 'O' });
  assert.deepEqual(kept, ['a']);
});

test('plural items merge by category', () => {
  const base = { items: { server: { one: '{count} Server', other: '{count} Server' } } };
  const ours = { items: { server: { one: '{count} Server', other: '{count} Servers (ours)' } } };
  const theirs = { items: { server: { one: '{count} Server (theirs)', other: '{count} Server', few: 'F' } } };

  const { merged } = mergeTranslations(base, ours, theirs);

  assert.deepEqual(merged.items.server, {
    one: '{count} Server (theirs)',
    other: '{count} Servers (ours)',
    few: 'F',
  });
});

test('git runs it on the files: the merge lands in %A, unreadable sides fail', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'merge-translations-'));
  const write = (name, value) => {
    const file = path.join(dir, name);
    fs.writeFileSync(file, typeof value === 'string' ? value : JSON.stringify(value));
    return file;
  };

  try {
    const base = write('base.json', '');
    const ours = write('ours.json', { a: 'A', mine: 'M' });
    const theirs = write('theirs.json', { a: 'A', b: 'B' });
    const script = fileURLToPath(new URL('./merge-translations.mjs', import.meta.url));

    execFileSync(process.execPath, [script, base, ours, theirs, 'de.json'], { stdio: 'pipe' });
    assert.equal(fs.readFileSync(ours, 'utf8'), `${JSON.stringify({ a: 'A', mine: 'M', b: 'B' }, null, 2)}\n`);

    const broken = write('broken.json', '{ "a": ');
    const before = fs.readFileSync(broken, 'utf8');
    const stderr = console.error;
    console.error = () => {};
    try {
      assert.equal(main([base, broken, theirs, 'de.json']), 1);
    } finally {
      console.error = stderr;
    }
    assert.equal(fs.readFileSync(broken, 'utf8'), before);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('a real git merge of a translation file has no conflict', (t) => {
  try {
    execFileSync('git', ['--version'], { stdio: 'pipe' });
  } catch {
    t.skip('git is not installed');
    return;
  }

  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'merge-translations-git-'));
  const script = fileURLToPath(new URL('./merge-translations.mjs', import.meta.url));
  const git = (...args) =>
    execFileSync('git', ['-c', 'user.name=test', '-c', 'user.email=test@example.com', ...args], {
      cwd: dir,
      stdio: 'pipe',
    }).toString();
  const write = (value) => fs.writeFileSync(path.join(dir, 'de.json'), `${JSON.stringify(value, null, 2)}\n`);

  try {
    git('init', '-q', '-b', 'main');
    git('config', 'merge.translations.driver', `"${process.execPath}" "${script}" %O %A %B %P`);
    fs.writeFileSync(path.join(dir, '.gitattributes'), '*.json merge=translations\n');
    write({ a: 'A', b: 'B', c: 'C' });
    git('add', '.');
    git('commit', '-q', '-m', 'base');

    git('checkout', '-q', '-b', 'upstream');
    write({ a: 'A (upstream)', b: 'B', c: 'C', d: 'D' });
    git('commit', '-q', '-am', 'upstream');

    git('checkout', '-q', 'main');
    write({ a: 'A', b: 'B (ours)', c: 'C', ours: 'O' });
    git('commit', '-q', '-am', 'ours');

    git('merge', '-q', '--no-edit', 'upstream');

    assert.deepEqual(JSON.parse(fs.readFileSync(path.join(dir, 'de.json'), 'utf8')), {
      a: 'A (upstream)',
      b: 'B (ours)',
      c: 'C',
      ours: 'O',
      d: 'D',
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
