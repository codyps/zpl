'use strict';
const wasm = require('./pkg/zpl_wasm.js');

function integer(value, name, minimum = 1) {
  if (value !== undefined && (!Number.isInteger(value) || value < minimum || value > 0xffffffff)) {
    throw new RangeError(`${name} must be an integer from ${minimum} to 4294967295`);
  }
  return value;
}

function render(input, options = {}) {
  if (typeof input === 'string') input = Buffer.from(input, 'utf8');
  if (!(input instanceof Uint8Array)) throw new TypeError('input must be a string, Buffer, or Uint8Array');
  if (options === null || typeof options !== 'object' || Array.isArray(options)) {
    throw new TypeError('options must be an object');
  }
  const { format = 'png', profile = 'zd621', width, height, dpi, label, resolveFont, limits = {} } = options;
  if (!['png', 'svg', 'pdf'].includes(format)) throw new TypeError('format must be png, svg, or pdf');
  if (!['specification', 'zd621', 'zq610-plus'].includes(profile)) throw new TypeError('Unknown rendering profile');
  if (resolveFont !== undefined && typeof resolveFont !== 'function') {
    throw new TypeError('resolveFont must be a synchronous function');
  }
  if (limits === null || typeof limits !== 'object' || Array.isArray(limits)) {
    throw new TypeError('limits must be an object');
  }
  for (const key of Object.keys(limits)) {
    if (!['inputBytes', 'fontBytes'].includes(key)) throw new TypeError(`Unknown limit: ${key}`);
  }
  const result = wasm.render_encoded(input, integer(width, 'width'), integer(height, 'height'),
    integer(dpi, 'dpi'), profile, format, integer(label, 'label', 0),
    integer(limits.inputBytes, 'limits.inputBytes', 0),
    integer(limits.fontBytes, 'limits.fontBytes', 0), resolveFont);
  try {
    return {
      data: Buffer.from(result.take_body()),
      format,
      width: result.width,
      height: result.height,
      labels: result.labels,
      warnings: result.take_warnings(),
    };
  } finally {
    result.free();
  }
}

exports.render = render;
exports.libraryVersion = wasm.library_version;

const elementKinds = ['before_first_command', 'format_command', 'control_command', 'control_character'];

class ParseError extends Error {
  constructor(offset, kind) {
    super(`ZPL framing error at byte ${offset}: ${kind}`);
    this.name = 'ParseError';
    this.offset = offset;
    this.kind = kind;
  }
}

function syntaxByte(value, name, fallback) {
  if (value === undefined) return fallback;
  if (!Number.isInteger(value) || value < 0 || value > 255) {
    throw new RangeError(`${name} must be an integer from 0 to 255`);
  }
  return value;
}

function parse(input, options = {}) {
  if (typeof input === 'string') input = Buffer.from(input, 'utf8');
  if (!(input instanceof Uint8Array)) throw new TypeError('input must be a string, Buffer, or Uint8Array');
  if (options === null || typeof options !== 'object' || Array.isArray(options)) {
    throw new TypeError('options must be an object');
  }
  const { syntax = {} } = options;
  if (syntax === null || typeof syntax !== 'object' || Array.isArray(syntax)) {
    throw new TypeError('syntax must be an object');
  }
  const result = wasm.parse_bytes(input,
    syntaxByte(syntax.formatPrefix, 'syntax.formatPrefix', 94),
    syntaxByte(syntax.controlPrefix, 'syntax.controlPrefix', 126),
    syntaxByte(syntax.delimiter, 'syntax.delimiter', 44));
  try {
    if (result.error_offset !== undefined) {
      throw new ParseError(result.error_offset, result.error_kind());
    }
    const spans = result.take_spans();
    const elements = [];
    for (let i = 0; i < spans.length; i += 3) {
      const offset = spans[i + 1];
      elements.push({
        kind: elementKinds[spans[i]],
        offset,
        data: Buffer.from(input.subarray(offset, offset + spans[i + 2])),
      });
    }
    return {
      elements,
      syntax: {
        formatPrefix: result.format_prefix,
        controlPrefix: result.control_prefix,
        delimiter: result.delimiter,
      },
    };
  } finally {
    result.free();
  }
}

exports.parse = parse;
exports.ParseError = ParseError;
