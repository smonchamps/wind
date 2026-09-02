"""E0 — dérive le dictionnaire identifiant → identifiant depuis tokens.csv.
Entrée : defs-*.tsv (inventaire), tokens.csv (glossaire par segment), fr-words.txt.
Sortie : dictionary.csv (layer, old, new, occurrences, files), unmapped.txt
(segments français sans entrée), collisions.txt, test-names.txt (phrases de test,
traduites à la main à E3, hors dictionnaire)."""
import os, re, csv, collections
OUT = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
FR = set(open(os.path.join(OUT, 'fr-words.txt'), encoding='utf-8').read().split())
tok = {}
for name in ['tokens.csv']:
    for row in csv.reader(open(os.path.join(OUT, name), encoding='utf-8')):
        if not row or row[0].startswith('#') or row[0] == 'fr': continue
        tok[row[0].strip().replace(' ', '_')] = row[1].strip()
# phrases multi-segments d'abord (longest match)
phrases = sorted([k for k in tok if '_' in k], key=lambda k: -k.count('_'))
IDENT = set()
KEEP_AS_IS = set('message messages date dates total version page pages archive simple pause budget phase phases journal queue note notes trace traces migration notification notifications secret session sessions cycle horizon menu toast theme themes echo echos nav conv inv seed uid pad ctx perf crash format chips kicker'.split())

def segs(ident):
    return [p for p in re.sub(r'([a-z0-9])([A-Z])', r'\1_\2', re.sub(r'([A-Z]+)([A-Z][a-z])', r'\1_\2', ident)).lower().split('_') if p]

def style(ident):
    if ident.isupper() or (ident.upper() == ident and '_' in ident): return 'UPPER'
    if '_' in ident or ident.islower(): return 'snake'
    if ident[0].isupper(): return 'Pascal'
    return 'camel'

def rebuild(parts, st):
    parts = [p for p in '_'.join(parts).split('_') if p]
    if st == 'UPPER': return '_'.join(p.upper() for p in parts)
    if st == 'snake': return '_'.join(parts)
    if st == 'Pascal': return ''.join(p.capitalize() for p in parts)
    return parts[0] + ''.join(p.capitalize() for p in parts[1:])

def translate(ident):
    s = segs(ident); out = []; unmapped = []; i = 0; changed = False
    while i < len(s):
        hit = None
        for ph in phrases:
            n = ph.count('_') + 1
            if '_'.join(s[i:i + n]) == ph: hit = (ph, n); break
        if hit:
            out.append(tok[hit[0]]); i += hit[1]; changed = True; continue
        w = s[i]
        if w in tok: out.append(tok[w]); changed = changed or tok[w] != w
        else:
            out.append(w)
            if w in FR and w not in KEEP_AS_IS: unmapped.append(w)
        i += 1
    return rebuild(out, style(ident)), unmapped, changed

rows = []; unmapped = collections.Counter(); tests = []
for layer in ['rust', 'ui', 'e2e-scripts']:
    for line in open(os.path.join(OUT, f'defs-{layer}.tsv'), encoding='utf-8').read().splitlines()[1:]:
        old, occ, files = line.split('\t')
        s = segs(old)
        if layer == 'rust' and len(s) >= 5 and old.islower() and int(occ) <= 2:
            tests.append(old); continue
        new, um, changed = translate(old)
        for w in um: unmapped[w] += 1
        if not changed or new == old: continue
        rows.append((layer, old, new, int(occ), files))
# collisions : deux anciens → même nouveau, ou nouveau déjà existant dans le code
existing = set()
for base, exts in [('crates', {'.rs'}), ('apps/desktop/src', {'.rs'}), ('apps/desktop/ui-v2/src', {'.js', '.svelte'}), ('e2e', {'.js', '.mjs'}), ('scripts', {'.mjs', '.ps1'})]:
    for root, dirs, fs in os.walk(os.path.join(ROOT, base)):
        dirs[:] = [d for d in dirs if d not in {'node_modules', 'test-results', 'target'}]
        for f in fs:
            if os.path.splitext(f)[1] in exts:
                existing |= set(re.findall(r'\b[A-Za-z_][A-Za-z0-9_]{2,}\b', open(os.path.join(root, f), encoding='utf-8', errors='replace').read()))
bynew = collections.defaultdict(list)
for layer, old, new, occ, files in rows: bynew[(layer, new)].append(old)
with open(os.path.join(OUT, 'collisions.txt'), 'w', encoding='utf-8', newline='\n') as w:
    for (layer, new), olds in sorted(bynew.items()):
        if len(olds) > 1: w.write(f'DOUBLE {layer} {new} <- {", ".join(olds)}\n')
        elif new in existing and new not in [o for o in olds]: w.write(f'EXISTS {layer} {new} <- {olds[0]}\n')
with open(os.path.join(OUT, 'dictionary.csv'), 'w', encoding='utf-8', newline='\n') as w:
    cw = csv.writer(w); cw.writerow(['layer', 'old', 'new', 'occurrences', 'files'])
    for r in sorted(rows, key=lambda r: (r[0], -r[3], r[1])): cw.writerow(r)
open(os.path.join(OUT, 'unmapped.txt'), 'w', encoding='utf-8', newline='\n').write('\n'.join(f'{c} {w}' for w, c in unmapped.most_common()))
open(os.path.join(OUT, 'test-names.txt'), 'w', encoding='utf-8', newline='\n').write('\n'.join(tests))
# --- clés de catalogue et contrat DOM (data-testid, classes CSS, seams e2e) ---
def tr_hyphen(name):
    new, _, changed = translate(name.replace('-', '_'))
    return new.replace('_', '-'), changed
keys = re.findall(r"^\s+'([a-zA-Z]+(?:\.[a-zA-Z0-9]+)+)'", open(os.path.join(ROOT, 'apps/desktop/ui-v2/src/lib/catalogue.fr.js'), encoding='utf-8').read(), re.M)
with open(os.path.join(OUT, 'dictionary-keys.csv'), 'w', encoding='utf-8', newline='\n') as w:
    cw = csv.writer(w); cw.writerow(['old', 'new']); n = 0
    for k in keys:
        parts = k.split('.')
        newparts = [translate(p)[0] for p in parts]
        new = '.'.join(newparts)
        if new != k: cw.writerow([k, new]); n += 1
    print('catalogue keys renamed', n, '/', len(keys))
ui = ''
for root, dirs, fs in os.walk(os.path.join(ROOT, 'apps/desktop/ui-v2/src')):
    for f in fs:
        if f.endswith(('.svelte', '.js', '.css')): ui += open(os.path.join(root, f), encoding='utf-8', errors='replace').read()
testids = sorted(set(re.findall(r'data-testid="([^"{]+)"', ui)) | set(m + '-' for m in re.findall(r'data-testid=\{`([a-z-]+)-\$', ui)))
classes = sorted(set(re.findall(r'^\s*\.([a-z][a-z0-9-]+)', ui, re.M)))
seams = sorted(set(re.findall(r'__e2e[A-Za-z]+', ui)))
with open(os.path.join(OUT, 'dictionary-dom.csv'), 'w', encoding='utf-8', newline='\n') as w:
    cw = csv.writer(w); cw.writerow(['kind', 'old', 'new']); n = 0
    for t in testids:
        new, ch = tr_hyphen(t.rstrip('-'))
        if ch: cw.writerow(['testid', t.rstrip('-'), new]); n += 1
    for c in classes:
        new, ch = tr_hyphen(c)
        if ch: cw.writerow(['class', c, new]); n += 1
    for s in seams:
        new, _, ch = translate(s[5:])
        if ch: cw.writerow(['seam', s, '__e2e' + new[0].upper() + new[1:]]); n += 1
    print('dom renamed', n, 'of', len(testids) + len(classes) + len(seams))
print(f'dictionary rows={len(rows)} tests(hand)={len(tests)} unmapped segments={len(unmapped)} collisions={sum(1 for _ in open(os.path.join(OUT, "collisions.txt"), encoding="utf-8"))}')
