// RETOURS-14 R3 (D4): no more em dash (U+2014) in the
// shipped texts — the net sweeps BOTH catalogs, all
// values (date tables included), and rejects the character. The
// replacements live case by case in the catalogs; the "—"
// empty-state glyphs (avatar, version) are NOT catalog
// texts and stay out of scope (D4).
import test from 'node:test';
import assert from 'node:assert/strict';
import { FR } from '../apps/desktop/ui-v2/src/lib/catalog.fr.js';
import { EN } from '../apps/desktop/ui-v2/src/lib/catalog.en.js';

for (const [name, table] of [['FR', FR], ['EN', EN]]) {
  test(`catalog ${name}: no em dash in the texts`, () => {
    const offenders = Object.entries(table)
      .filter(([, value]) => JSON.stringify(value).includes('—'))
      .map(([key]) => key);
    assert.deepEqual(offenders, [], `em dash in: ${offenders.join(', ')}`);
  });
}
