#!/usr/bin/env python3
"""Stage a copy of Tests/Consumer whose main.swift runs the README examples.

Import declarations are hoisted and deduplicated; each other block runs in its
own `do` scope, so examples may reuse names. Top-level code in main.swift may
throw, and an uncaught error fails the run:
https://docs.swift.org/swift-book/documentation/the-swift-programming-language/errorhandling/
A `<!-- readme-test: skip -->` line directly before a fence excludes a block.
Fenced code blocks: https://spec.commonmark.org/0.31.2/#fenced-code-blocks

Usage: readme-consumer.py DESTINATION, then `swift run --package-path DESTINATION`
with ZPL_PACKAGE_PATH set as for Tests/Consumer.
"""
import argparse
from pathlib import Path
import re
import shutil

ROOT = Path(__file__).resolve().parent
SKIP = '<!-- readme-test: skip -->'
FENCE = re.compile(r'^```swift[ \t]*\n(.*?)^```[ \t]*$', re.MULTILINE | re.DOTALL)


def program(readmes):
    """Combine the runnable blocks of (name, text) READMEs into one main.swift."""
    imports, bodies = [], []
    for name, text in readmes:
        for match in FENCE.finditer(text):
            before = text[:match.start()].rstrip('\n').rsplit('\n', 1)[-1].strip()
            if before == SKIP:
                continue
            line = text.count('\n', 0, match.start()) + 1
            body = []
            for source in match.group(1).splitlines():
                if source.startswith('import '):
                    if source not in imports:
                        imports.append(source)
                else:
                    body.append('    ' + source if source else '')
            bodies.append(f'// {name}:{line}\ndo {{\n' + '\n'.join(body) + '\n}')
    if not bodies:
        raise SystemExit('READMEs have no runnable Swift examples')
    return '\n'.join(imports) + '\n\n' + '\n\n'.join(bodies) + '\nprint("README examples passed")\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('destination', type=Path)
    args = parser.parse_args()
    shutil.copytree(ROOT / 'Tests/Consumer', args.destination, ignore=shutil.ignore_patterns('.build'))
    # This package's README and the workspace README's Swift section.
    source = program([
        ('zpl-swift/README.md', (ROOT / 'README.md').read_text(encoding='utf-8')),
        ('README.md', (ROOT.parent / 'README.md').read_text(encoding='utf-8')),
    ])
    (args.destination / 'Sources/Consumer/main.swift').write_text(source, encoding='utf-8')


if __name__ == '__main__':
    main()
