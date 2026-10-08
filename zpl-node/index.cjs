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
  const { format = 'png', profile = 'zd621', width, height, dpi, label } = options;
  if (!['png', 'svg', 'pdf'].includes(format)) throw new TypeError('format must be png, svg, or pdf');
  if (!['specification', 'zd621', 'zq610-plus'].includes(profile)) throw new TypeError('Unknown rendering profile');
  const result = wasm.render_encoded(input, integer(width, 'width'), integer(height, 'height'),
    integer(dpi, 'dpi'), profile, format, integer(label, 'label', 0));
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
