#!/usr/bin/env python3
import argparse
import json
import sys
from pathlib import Path


parser = argparse.ArgumentParser()
parser.add_argument("image", type=Path)
parser.add_argument("candidate", type=Path)
parser.add_argument("--catalog-bin")
parser.add_argument("--base-url")
parser.add_argument("--model")
parser.add_argument("--device")
parser.add_argument("--lang")
parser.add_argument("--page", type=int, required=True)
args = parser.parse_args()

if args.model == "fail-page-1" and args.page == 1:
    print("controlled page failure", file=sys.stderr)
    raise SystemExit(2)
if not args.image.read_bytes().startswith(b"\x89PNG\r\n\x1a\n"):
    print("input is not a PNG", file=sys.stderr)
    raise SystemExit(2)

json.dump({
    "schema_version": 1,
    "observation": {"source_page_index": args.page},
    "comparison": {"verdict": {"status": "preserved"}},
}, sys.stdout, separators=(",", ":"))
sys.stdout.write("\n")
