// Run the JavaScript examples in this package's README and the workspace
// README, each README as one ES module in a child process.
// Import declarations are hoisted and merged; each block body runs in its own
// block scope so examples may reuse names. A `<!-- readme-test: skip -->` line
// directly before a fence excludes that block.
// Fenced code blocks: https://spec.commonmark.org/0.31.2/#fenced-code-blocks
// Block-scoped declarations: https://tc39.es/ecma262/#sec-block
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

// The package README and the workspace README's Node.js section.
const readmes = [
  ['zpl-node/README.md', fileURLToPath(new URL('../README.md', import.meta.url))],
  ['README.md', fileURLToPath(new URL('../../README.md', import.meta.url))],
];
// The README font example reads brand.ttf (it needs Latin letters, unlike the
// glyph probes) and a large-job example reads a ZPL file.
const brandFont = fileURLToPath(new URL(
  '../../zpl/tests/fixtures/external-fonts-zd621-v1/ComparisonHerosCondensedBold.ttf',
  import.meta.url,
));
const SKIP = '<!-- readme-test: skip -->';
const FENCE = /^```js[ \t]*\n([\s\S]*?)^```[ \t]*$/gm;
const IMPORT = /^import\s+(?:(\w+)\s*,?\s*)?(?:\{([^}]*)\})?\s*from\s+'([^']+)';?[ \t]*$/;

function program(name, text) {
  const imports = new Map();
  const bodies = [];
  for (const match of text.matchAll(FENCE)) {
    const before = text.slice(0, match.index).trimEnd().split('\n').at(-1).trim();
    if (before === SKIP) continue;
    const line = text.slice(0, match.index).split('\n').length;
    const body = [];
    for (const source of match[1].split('\n')) {
      if (!source.startsWith('import ')) {
        body.push(source);
        continue;
      }
      const parsed = IMPORT.exec(source);
      assert.ok(parsed, `${name}:${line}: unsupported import form: ${source}`);
      const [, defaultName, named = '', specifier] = parsed;
      const entry = imports.get(specifier) ?? { defaultName: undefined, named: new Set() };
      if (defaultName) entry.defaultName = defaultName;
      for (const name of named.split(',').map((name) => name.trim()).filter(Boolean)) {
        entry.named.add(name);
      }
      imports.set(specifier, entry);
    }
    bodies.push(`// ${name}:${line}\n{\n${body.join('\n')}\n}`);
  }
  const head = [...imports].map(([specifier, { defaultName, named }]) => {
    // Resolve the package's self-reference from this checkout, not node_modules.
    const target = specifier === '@codyps/zpl' ? import.meta.resolve(specifier) : specifier;
    const clauses = [defaultName, named.size ? `{ ${[...named].join(', ')} }` : undefined];
    return `import ${clauses.filter(Boolean).join(', ')} from ${JSON.stringify(target)};`;
  });
  return { source: `${head.join('\n')}\n\n${bodies.join('\n\n')}\n`, blocks: bodies.length };
}

for (const [name, readme] of readmes) {
  test(`${name} examples run`, () => {
    const { source, blocks } = program(name, readFileSync(readme, 'utf8'));
    assert.ok(blocks > 0, `${name} has runnable JavaScript examples`);
    const directory = mkdtempSync(join(tmpdir(), 'zpl-node-readme-'));
    try {
      copyFileSync(brandFont, join(directory, 'brand.ttf'));
      writeFileSync(join(directory, 'label-with-font.zpl'), '^XA^FO20,20^A0N,32,0^FDHello^FS^XZ');
      writeFileSync(join(directory, 'readme.mjs'), source);
      execFileSync(process.execPath, ['readme.mjs'], { cwd: directory, stdio: ['ignore', 'ignore', 'inherit'] });
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });
}
