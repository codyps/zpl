#!/usr/bin/env python3
"""Rebuild the command/page index from the checked-in Zebra guide (pdftotext required)."""
from pathlib import Path
import re
import subprocess

root = Path(__file__).resolve().parent.parent
text = subprocess.check_output(
    ["pdftotext", "-layout", str(root / "docs/zpl-zbi2-pg-en.pdf"), "-"], text=True
)
# Revision P1134473-11EN: TOC pp. 3-46; ZPL, network and RFID pp. 48-441.
toc = "\n".join(text.split("\f")[2:46])
commands = {}
for line in toc.splitlines():
    match = re.fullmatch(r"\s*([\^~].*?)\.{3,}\s*(\d+)\s*", line)
    if match and 48 <= int(match[2]) <= 441:
        for command in re.findall(r"[\^~][A-Z0-9@]{1,2}", match[1]):
            if command in commands:
                raise ValueError(f"Duplicate command: {command}")
            commands[command] = int(match[2])
if len(commands) != 223:
    raise ValueError(f"Expected 223 TOC spellings for this revision, found {len(commands)}")
# The ZB64 appendix names DL, but the guide has no standalone syntax entry for it.
commands["~DL"] = 1604
output = "# Command\tPDF page (P1134473-11EN Rev A)\n"
output += "".join(f"{command}\t{page}\n" for command, page in commands.items())
(root / "docs/zpl-command-index.tsv").write_text(output)
print(f"Indexed {len(commands)} spellings (223 TOC entries plus ~DL from the appendix)")
