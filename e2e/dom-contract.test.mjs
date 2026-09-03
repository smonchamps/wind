// PLAN-BASCULE-ANGLAIS E5d (2026-09-03): the two sides of the DOM
// contract. Every test id a spec or an e2e tool selects must be rendered
// by the UI — a static `data-testid`, the `testid` prop of `Menu`, or an
// id built on a rendered prefix (`bar-{g.action}`). A spec id the UI
// never renders is a dead assertion: a locator that matches nothing, a
// `toHaveCount(0)` that passes for the wrong reason. The E5d pass found
// one (`settings-panel`, a fallback selector nobody had questioned since
// a77ab47); the check was a one-off report of the applier — it lives
// here from now on, in the test script, so a rename on one side alone
// turns red.
import test from 'node:test';
import assert from 'node:assert/strict';
import { specIdsAgainstUi } from '../scripts/rename/apply-dom.mjs';

test('every id the specs select is rendered by the UI', () => {
  const { renderedIds, stale } = specIdsAgainstUi();
  assert.ok(renderedIds.size > 300, `the UI renders ${renderedIds.size} ids — the scan is broken`);
  const lines = [...stale].map(([id, files]) => `${id}  (${[...files].join(', ')})`);
  assert.deepEqual(lines, [], `spec ids the UI does not render:\n${lines.join('\n')}`);
});
