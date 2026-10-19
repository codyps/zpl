#!/usr/bin/env python3
"""Build a portable shared library, with Zig for Linux libc baselines.

Requires cargo-zigbuild and Zig on Linux. Musl cdylibs must link libc dynamically:
https://github.com/philss/rustler_precompiled/blob/v0.9.0/PRECOMPILATION_GUIDE.md#additional-configuration-before-build
https://github.com/rust-cross/cargo-zigbuild#specify-glibc-version
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess


def build(crate, target, output, static=False):
    env = os.environ.copy()
    command = ['cargo', 'build']
    build_target = target
    if '-linux-' in target:
        command = ['cargo', 'zigbuild']
        if target.endswith('gnu') or target.endswith('gnueabihf'):
            build_target += '.2.28'
        else:
            # A static musl CRT disables cdylib output. Never apply this flag to
            # unrelated workspace builds or to the Windows/macOS slices.
            env['RUSTFLAGS'] = env.get('RUSTFLAGS', '') + ' -C target-feature=-crt-static'
    subprocess.run([*command, '--locked', '--release', '-p', crate, '--target', build_target],
                   env=env, check=True)
    library = crate.replace('-', '_')
    if static:
        library = library + '.lib' if 'windows' in target else f'lib{library}.a'
    elif 'windows' in target:
        library += '.dll'
    elif 'apple' in target:
        library = f'lib{library}.dylib'
    else:
        library = f'lib{library}.so'
    source = Path(env.get('CARGO_TARGET_DIR', 'target')) / target / 'release' / library
    output.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, output)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('crate', choices=['zpl_elixir', 'zpl-c'])
    parser.add_argument('target')
    parser.add_argument('output', type=Path)
    parser.add_argument('--static', action='store_true', help='Stage the Rust static library')
    args = parser.parse_args()
    build(args.crate, args.target, args.output, args.static)
