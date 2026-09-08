#!/usr/bin/env python3

# TODO: Drop this script once cargo --artifact-dir gets stabilized, so we don't need the second copy step anymore
# https://github.com/rust-lang/cargo/issues/6790

import argparse
import shutil
import subprocess
import sys

parser = argparse.ArgumentParser()
parser.add_argument('--cargo')
parser.add_argument('--binary')
parser.add_argument('--output')
parser.add_argument('cargo_options', nargs='*')
args = parser.parse_args()

# 1. Cargo build
result = subprocess.run([args.cargo, 'build'] + args.cargo_options)
if result.returncode != 0:
    sys.exit(result.returncode)

# 2. Copy binary to the location meson expects
shutil.copy2(args.binary, args.output)
