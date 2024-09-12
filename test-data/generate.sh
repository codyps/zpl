#! /usr/bin/env bash
set -eu -o pipefail

d="$(dirname "$0")"

out="$d"/_gen
mkdir -p "$out"
for f in "$d"/*.zpl; do
	dpmm=8dpmm
	size=4x6
	label_idx=0
	b="$(basename "$f" .zpl)"
	curl -H "Accept: application/pdf" -v "http://api.labelary.com/v1/printers/$dpmm/labels/$size/$label_idx/" \
		--data "@$f" -o "$out/$b.$dpmm.$size.$label_idx.pdf"
done

