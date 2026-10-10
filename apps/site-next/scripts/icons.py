"""Regenerates components/icon-data.ts from the icon list in the plain site
(apps/site/src/common.ts), which stays the source of truth."""
import json, re
src = open('../site/src/common.ts').read()
imports = dict(re.findall(r"import (\w+) from '@phosphor-icons/core/assets/regular/([\w-]+)\.svg\?raw'", src))
block = re.search(r'const ICONS: Record<string, string> = \{(.*?)\n\}', src, re.S).group(1)
icons = {}
for line in block.strip().split('\n'):
    line = line.strip().rstrip(',')
    k, v = ([x.strip() for x in line.split(':', 1)] if ':' in line else (line, line))
    icons[k.strip("'")] = open(f'node_modules/@phosphor-icons/core/assets/regular/{imports[v]}.svg').read().strip()
open('components/icon-data.ts', 'w').write(
    "// Generated from @phosphor-icons/core (regular), the same set the plain site uses.\n"
    "// Run scripts/icons.py to refresh.\nexport const ICONS: Record<string, string> = "
    + json.dumps(icons, indent=2) + ";\n")
