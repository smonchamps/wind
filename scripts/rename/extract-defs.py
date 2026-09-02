"""E0 — inventaire brut des définitions à renommer, par couche.
Sortie : scratchpad/defs-<couche>.tsv  (identifiant \t occurrences \t fichiers)
Et scratchpad/files-to-rename.txt (fichiers aux noms français)."""
import os, re, collections, sys
ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
OUT = os.path.dirname(os.path.abspath(__file__))
os.chdir(ROOT)
EXCL = {'target', 'node_modules', 'dist', 'spikes', '.git', 'worktrees', 'test-results', 'gen'}
FR = set(open(os.path.join(OUT, 'fr-words.txt'), encoding='utf-8').read().split())
str_re = re.compile(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|`(?:\\.|[^`\\])*`')
com_re = re.compile(r'//[^\n]*|/\*.*?\*/|<!--.*?-->', re.S)
def toks(i): return [p for p in re.sub(r'([a-z0-9])([A-Z])', r'\1_\2', i).lower().split('_') if p]
def is_fr(i): return any(t in FR for t in toks(i))
DEF = re.compile(r'\b(?:fn|struct|enum|trait|mod|const|static|type|let|function|class|macro_rules!)\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)')
FIELD = re.compile(r'^\s+(?:pub(?:\([a-z]+\))?\s+)?([a-z_][a-z0-9_]*)\s*:\s*[A-Za-z&\[(]', re.M)  # struct fields (rust)
VARIANT = re.compile(r'^\s+([A-Z][A-Za-z0-9]*)\s*(?:\{|\(|,|$)', re.M)
JSDEF = re.compile(r'\b(?:const|let|var|function|class|export\s+(?:const|let|function|class|async function)|async function)\s+([A-Za-z_$][A-Za-z0-9_$]*)')
JSKEY = re.compile(r'^\s*([a-zA-Z_$][A-Za-z0-9_$]*)\s*[:(]', re.M)  # object keys / methods
def scan(paths, exts, rust):
    defs = collections.defaultdict(lambda: [0, set()])
    for base in paths:
        for root, dirs, fs in os.walk(base):
            dirs[:] = [d for d in dirs if d not in EXCL]
            for f in fs:
                if os.path.splitext(f)[1] not in exts: continue
                p = os.path.join(root, f).replace('\\', '/')
                txt = open(p, encoding='utf-8', errors='replace').read()
                code = str_re.sub('""', com_re.sub('', txt))
                names = set()
                if rust:
                    names |= set(DEF.findall(code)) | set(FIELD.findall(code)) | set(VARIANT.findall(code))
                else:
                    names |= set(JSDEF.findall(code)) | set(JSKEY.findall(code))
                for n in names:
                    if len(n) < 3 or not is_fr(n): continue
                    occ = len(re.findall(r'\b' + re.escape(n) + r'\b', code))
                    defs[n][0] += occ; defs[n][1].add(p)
    return defs
layers = [('rust', ['crates', 'apps/desktop/src'], {'.rs'}, True),
          ('ui', ['apps/desktop/ui-v2/src'], {'.js', '.svelte'}, False),
          ('e2e-scripts', ['e2e', 'scripts'], {'.js', '.mjs', '.ps1', '.py'}, False)]
total = 0
for name, paths, exts, rust in layers:
    d = scan(paths, exts, rust)
    with open(os.path.join(OUT, f'defs-{name}.tsv'), 'w', encoding='utf-8', newline='\n') as w:
        w.write('identifiant\toccurrences\tfichiers\n')
        for n, (occ, files) in sorted(d.items(), key=lambda x: (-x[1][0], x[0])):
            w.write(f'{n}\t{occ}\t{";".join(sorted(files))}\n')
    print(f'{name}: {len(d)} définitions françaises')
    total += len(d)
print('total', total)
# fichiers aux noms français
with open(os.path.join(OUT, 'files-to-rename.txt'), 'w', encoding='utf-8', newline='\n') as w:
    for base in ['crates', 'apps/desktop/src', 'apps/desktop/ui-v2/src', 'e2e', 'scripts', 'docs', '.claude/skills', '.githooks']:
        for root, dirs, fs in os.walk(base):
            dirs[:] = [d for d in dirs if d not in EXCL and d != 'archives']
            for f in fs:
                stem = re.sub(r'\.(rs|js|mjs|svelte|ps1|py|md|html|css)$', '', f)
                if is_fr(stem.replace('-', '_')) or re.search(r'[éèàçù]', stem):
                    w.write(os.path.join(root, f).replace('\\', '/') + '\n')
print('files-to-rename.txt écrit')
