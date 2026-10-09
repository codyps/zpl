import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { createRequire } from 'node:module';
import { render, resolveRomFont } from '@codyps/zpl';

// Original constructed TrueType probe used by Rust font-download regressions.
// Zebra Programming Guide ~DT/~DU/~DY and ^A@/^CW:
// https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
const font = readFileSync(process.env.ZPL_TEST_FONT ??
  new URL('../../zpl/tests/fixtures/truetype-regression/font-probes-20261002/probe.ttf', import.meta.url));
const options = { width: 100, height: 100, profile: 'specification' };
const label = '^XA^FO10,10^A@N,24,31,R:TEST.TTF^FD!!^FS^XZ';
const download = Buffer.concat([
  Buffer.from(`~DYR:TEST.TTF,B,T,${font.length},,`), font,
]);
const inline = Buffer.concat([download, Buffer.from(label)]);

test('resolver matches inline TrueType across adapters and preserves byte views', () => {
  const padded = Buffer.concat([Buffer.from('junk'), font, Buffer.from('junk')]);
  const view = new Uint8Array(padded.buffer, padded.byteOffset + 4, font.length);
  for (const format of ['png', 'svg', 'pdf']) {
    const calls = [];
    const result = render(label + label, { ...options, format, resolveFont(name) {
      calls.push(name);
      return view;
    } });
    assert.deepEqual(calls, ['R:TEST.TTF']);
    assert.deepEqual(result, render(Buffer.concat([inline, Buffer.from(label)]), { ...options, format }));
  }
  assert.throws(() => render(label, options), /unresolved named font/);
});

test('resolved bytes remain owned across further callbacks and buffer mutation', () => {
  const temporary = Buffer.from(font);
  const second = label.replace('R:TEST.TTF', 'R:SECOND.TTF');
  const calls = [];
  const actual = render(label + second + label, { ...options, format: 'pdf', resolveFont(name) {
    calls.push(name);
    if (name === 'R:TEST.TTF') return temporary;
    temporary.fill(0);
    return font;
  } });
  assert.deepEqual(calls, ['R:TEST.TTF', 'R:SECOND.TTF']);
  assert.deepEqual(actual, render(Buffer.concat([inline, Buffer.from(label + label)]), {
    ...options, format: 'pdf',
  }));
});

test('CW canonicalizes names; inline downloads take precedence', () => {
  const alias = '^CWZ,test.ttf^XA^FO10,10^AZN,24,31^FD!!^FS^XZ';
  assert.deepEqual(render(alias, { ...options, resolveFont(name) {
    assert.equal(name, 'R:TEST.TTF');
    return font;
  } }).data, render(inline, options).data);
  assert.deepEqual(render(inline, { ...options, resolveFont() {
    throw new Error('must not resolve a downloaded font');
  } }).data, render(inline, options).data);
});

test('custom resolvers replace ROM lookup and can explicitly delegate', () => {
  assert.equal(createRequire(import.meta.url)('@codyps/zpl').resolveRomFont, resolveRomFont);
  const rom = '^XA^A@N,12,12,Z:E12.FNT^FDABC^FS^XZ';
  const expected = render(rom, options);
  for (const value of [null, undefined]) {
    assert.throws(() => render(rom, { ...options, resolveFont: () => value }), /unresolved named font/);
    assert.throws(() => render(label, { ...options, resolveFont: () => value }), /unresolved named font/);
  }
  assert.deepEqual(render(rom, { ...options, resolveFont: resolveRomFont }), expected);
  const face = resolveRomFont('z:e12.fnt');
  assert.ok(face);
  for (let i = 0; i < 2; i++) {
    assert.deepEqual(render(rom.replace('Z:E12.FNT', 'R:ALIAS.FNT'), {
      ...options, resolveFont: () => face,
    }), expected);
  }
  const overridden = render(label.replace('R:TEST.TTF', 'Z:E12.FNT'), {
    ...options, resolveFont: () => font,
  });
  assert.deepEqual(overridden, render(inline, options));
  const resident = '^XA^AAN,9,5^FDABC^FS^XZ';
  assert.deepEqual(render(resident, { ...options, resolveFont: () => null }), render(resident, options));
  assert.equal(resolveRomFont('R:UNKNOWN.TTF'), null);
  assert.throws(() => resolveRomFont('../font.ttf'), /invalid named font/);
  assert.throws(() => resolveRomFont(null), TypeError);
  assert.throws(() => render(rom, { ...options, resolveFont: () => ({}) }), /resolveFont must return/);
});

test('resolver errors and nested renders are render-scoped', () => {
  for (const value of [[], 'font.ttf', Promise.resolve(font), 42]) {
    assert.throws(() => render(label, { ...options, resolveFont: () => value }), /resolveFont must return/);
  }
  assert.throws(() => render(label, { ...options, resolveFont() {
    throw new Error('font loading failed');
  } }), /font loading failed/);
  assert.throws(() => render(label, { ...options, resolveFont: () => Buffer.from('invalid') }), /TrueType|truncated/);
  const expected = render(inline, options);
  assert.deepEqual(render(label, { ...options, resolveFont() {
    assert.deepEqual(render(label, { ...options, resolveFont: () => font }), expected);
    return font;
  } }), expected);
});

test('large binary and hex inline fonts work with explicit budgets', () => {
  // SFNT tables retain their offsets; trailing padding produces a valid >1 MiB
  // font without importing a large third-party font fixture.
  const large = Buffer.alloc(1_048_577);
  font.copy(large);
  for (const data of [
    Buffer.concat([Buffer.from(`~DYR:TEST.TTF,B,T,${large.length},,`), large, Buffer.from(label)]),
    Buffer.from(`~DUR:TEST.TTF,${large.length},${large.toString('hex')}${label}`),
  ]) {
    assert.throws(() => render(data, options), /1 MiB renderer limit/);
    const raised = { ...options, limits: { inputBytes: data.length, fontBytes: large.length } };
    assert.deepEqual(render(data, raised), render(inline, options));
    assert.throws(() => render(data, { ...raised, limits: { inputBytes: data.length - 1 } }), /configured renderer limit/);
    assert.throws(() => render(data, { ...raised, limits: { inputBytes: data.length, fontBytes: large.length - 1 } }), /font.*limit|budget/);
  }
});

test('validates callback and budget options before Wasm coercion', () => {
  assert.throws(() => render(label, { resolveFont: 'file.ttf' }), TypeError);
  for (const limits of [null, [], 12, { unknown: 1 }]) {
    assert.throws(() => render(label, { limits }), TypeError);
  }
  for (const value of [-1, 1.5, NaN, Infinity, 2 ** 32, '100', null]) {
    for (const key of ['inputBytes', 'fontBytes']) {
      assert.throws(() => render(label, { limits: { [key]: value } }), RangeError);
    }
  }
  assert.throws(() => render(inline, { ...options, limits: { fontBytes: 0 } }), /font.*limit|budget/);
});
