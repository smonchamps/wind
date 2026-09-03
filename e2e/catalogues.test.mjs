// RETOURS-14 R3 (D4) : plus aucun tiret cadratin (U+2014) dans les
// textes expédiés — le filet balaie les DEUX catalogues, toutes les
// valeurs (tables de dates comprises), et refuse le caractère. Les
// remplacements vivent au cas par cas dans les catalogues ; les « — »
// glyphes de vide (avatar, version) ne sont PAS des textes de
// catalogue et restent hors périmètre (D4).
import test from 'node:test';
import assert from 'node:assert/strict';
import { FR } from '../apps/desktop/ui-v2/src/lib/catalog.fr.js';
import { EN } from '../apps/desktop/ui-v2/src/lib/catalog.en.js';

for (const [nom, table] of [['FR', FR], ['EN', EN]]) {
  test(`catalogue ${nom} : aucun tiret cadratin dans les textes`, () => {
    const fautifs = Object.entries(table)
      .filter(([, valeur]) => JSON.stringify(valeur).includes('—'))
      .map(([cle]) => cle);
    assert.deepEqual(fautifs, [], `cadratin dans : ${fautifs.join(', ')}`);
  });
}
