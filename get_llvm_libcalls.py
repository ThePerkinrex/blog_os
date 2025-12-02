#!/usr/bin/env python3
import re
import sys
from urllib.request import urlopen, Request
from urllib.error import URLError, HTTPError

URL = "https://raw.githubusercontent.com/llvm/llvm-project/main/llvm/include/llvm/IR/RuntimeLibcalls.td"

def fetch(url):
    req = Request(url, headers={"User-Agent": "python-urllib/3"})
    with urlopen(req, timeout=15) as resp:
        return resp.read().decode("utf-8", errors="replace")

try:
    td = fetch(URL)
except Exception as e:
    print("ERROR fetching:", e, file=sys.stderr)
    sys.exit(1)

names = set()

# Pattern A: def IDENT : RuntimeLibcallImpl<...>
pat_def = re.compile(r'\bdef\s+([A-Za-z_]\w*)\s*:\s*RuntimeLibcallImpl\b', re.MULTILINE)
for m in pat_def.finditer(td):
    names.add(m.group(1))

# Pattern B: RuntimeLibcallImpl<"symbol", ...>
pat_string = re.compile(r'RuntimeLibcallImpl\s*<\s*"([^"]+)"', re.MULTILINE)
for m in pat_string.finditer(td):
    names.add(m.group(1))

# Print sorted unique names
for n in sorted(names):
    print(n)
