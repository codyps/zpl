import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { test } from 'node:test';
import { parse, ParseError } from '@codyps/zpl';

// Framing contract and Zebra reference: docs/parser-coverage.md in the source.
// https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
function roundTrip(input, options) {
  const bytes = typeof input === 'string' ? Buffer.from(input) : Buffer.from(input);
  const result = parse(input, options);
  assert.deepEqual(Buffer.concat(result.elements.map(e => e.data)), bytes);
  let offset = 0;
  for (const element of result.elements) {
    assert.equal(element.offset, offset);
    assert.ok(Buffer.isBuffer(element.data));
    offset += element.data.length;
  }
  return result;
}

test('ESM/CommonJS expose parsing, empty streams, and all element kinds', () => {
  const cjs = createRequire(import.meta.url)('@codyps/zpl');
  assert.equal(cjs.parse, parse);
  assert.equal(cjs.ParseError, ParseError);
  assert.deepEqual(parse('').elements, []);
  const result = roundTrip('hello\r\n^XA^ZZunknown\n~HS\x0f^XZ');
  assert.deepEqual(result.elements.map(e => e.kind), [
    'before_first_command', 'format_command', 'format_command',
    'control_command', 'control_character', 'format_command',
  ]);
  assert.deepEqual(result.syntax, { formatPrefix: 94, controlPrefix: 126, delimiter: 44 });
  assert.equal(result.elements[2].data.toString(), '^ZZunknown\n');
  roundTrip('é^XA^FD你好^FS^XZ');
});

test('binary payloads preserve every byte, typed-array offsets, and owned results', () => {
  const payload = Buffer.from(Array.from({ length: 256 }, (_, n) => n));
  const input = Buffer.concat([Buffer.from('^XA^GFB,256,256,32,'), payload, Buffer.from('^FS^XZ')]);
  const padded = Buffer.concat([Buffer.from('junk'), input, Buffer.from('junk')]);
  const view = new Uint8Array(padded.buffer, padded.byteOffset + 4, input.length);
  const result = roundTrip(view);
  assert.equal(result.elements.length, 4);
  assert.deepEqual(result.elements[1].data.subarray(-256), payload);
  view.fill(0);
  parse('^XA^XZ');
  assert.deepEqual(Buffer.concat(result.elements.map(e => e.data)), input);
  result.elements[0].data.fill(0);
  assert.equal(result.elements[1].data[0], 94);
});

test('initial syntax, in-stream changes, and continuation across complete streams', () => {
  const first = roundTrip('^CC!!CT?!CD;!XA!FO1;2!XZ');
  assert.deepEqual(first.syntax, { formatPrefix: 33, controlPrefix: 63, delimiter: 59 });
  const next = roundTrip('!XA?HS!XZ', { syntax: first.syntax });
  assert.deepEqual(next.elements.map(e => e.kind), ['format_command', 'control_command', 'format_command']);
  roundTrip('/XA/FO1;2/XZ', { syntax: { formatPrefix: 47, delimiter: 59 } });
});

test('encoded downloads and prefix-valued barcode operands stay framed', () => {
  roundTrip('~DUR:TEST.FNT,2,:B64:AAA=:0000^XA^XZ');
  const result = roundTrip('^XA^BXN,2,200,0,0,1,^,1^FDABC^FS^XZ');
  assert.equal(result.elements[1].data.toString(), '^BXN,2,200,0,0,1,^,1');
});

test('structured failures identify the failed command without interpreting its tail', () => {
  for (const [input, offset, kind] of [
    ['^XA^', 3, 'IncompleteCommand'],
    ['^XA^CC', 3, 'MissingSyntaxCharacter'],
    ['^XA^GFB,', 3, 'InvalidBinaryHeader'],
    ['^XA^GFB,x,1,1,', 3, 'InvalidBinaryLength'],
    ['é^GFB,20,20,1,^CC!', 2, 'TruncatedBinaryData'],
    ['~DUR:T.FNT,2,:B64:?', 0, 'InvalidEncodedData'],
    ['~DUR:T.FNT,2,:B64:AA', 0, 'IncompleteEncodedData'],
  ]) {
    assert.throws(() => parse(input), error => {
      assert.ok(error instanceof ParseError);
      assert.equal(error.name, 'ParseError');
      assert.equal(error.offset, offset);
      assert.equal(error.kind, kind);
      return true;
    });
  }
  assert.equal(parse('^XA^XZ').elements.length, 2);
});

test('validates input and syntax bytes before Wasm coercion', () => {
  for (const input of [null, {}, 42, [94, 88, 65]]) assert.throws(() => parse(input), TypeError);
  for (const options of [null, [], 1, { syntax: null }, { syntax: [] }]) {
    assert.throws(() => parse('', options), TypeError);
  }
  for (const key of ['formatPrefix', 'controlPrefix', 'delimiter']) {
    for (const value of [-1, 256, 1.5, NaN, Infinity, '^', null]) {
      assert.throws(() => parse('', { syntax: { [key]: value } }), RangeError);
    }
  }
  assert.equal(parse('', { syntax: { delimiter: 0 } }).syntax.delimiter, 0);
});
