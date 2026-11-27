"""Run the Python examples in the package and workspace READMEs.

Blocks in one README run in order in a shared namespace, like a REPL session,
inside a temporary working directory so example output files are discarded.
A ``<!-- readme-test: skip -->`` line directly before a fence excludes that
block. Fenced code blocks: https://spec.commonmark.org/0.31.2/#fenced-code-blocks
"""

import contextlib
import io
import os
import re
import tempfile
import unittest
from pathlib import Path

PACKAGE = Path(__file__).resolve().parent.parent
READMES = [PACKAGE / "README.md", PACKAGE.parent / "README.md"]
FENCE = re.compile(r"^```python[ \t]*\n(.*?)^```[ \t]*$", re.MULTILINE | re.DOTALL)
SKIP = "<!-- readme-test: skip -->"


def python_blocks(text):
    for match in FENCE.finditer(text):
        before = text[: match.start()].rstrip("\n").rsplit("\n", 1)[-1]
        line = text.count("\n", 0, match.start()) + 1
        yield line, match.group(1), before.strip() == SKIP


class ReadmeTest(unittest.TestCase):
    def test_readme_examples(self):
        for readme in READMES:
            if not readme.is_file():
                # Source distributions unpacked outside the checkout lack the workspace README.
                continue
            namespace = {"__name__": "__readme__"}
            with tempfile.TemporaryDirectory() as directory:
                previous = os.getcwd()
                os.chdir(directory)
                try:
                    for line, code, skipped in python_blocks(readme.read_text(encoding="utf-8")):
                        with self.subTest(readme=str(readme), line=line):
                            if skipped:
                                self.skipTest("marked readme-test: skip")
                            try:
                                with contextlib.redirect_stdout(io.StringIO()):
                                    exec(compile(code, f"{readme}:{line}", "exec"), namespace)
                            except ModuleNotFoundError as error:
                                if error.name and error.name.split(".")[0] == "zplkit":
                                    raise
                                # Optional third-party integrations such as Pillow.
                                self.skipTest(f"optional module {error.name} is not installed")
                finally:
                    os.chdir(previous)


if __name__ == "__main__":
    unittest.main()
