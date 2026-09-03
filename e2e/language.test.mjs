// PLAN-ENGLISH-SWITCH E5b, decision D4 (2026-09-02): English is the
// reference language of the interface — the first launch speaks English
// unless the system language is French, and a key missing from the
// active catalogue falls back to the English text. The decision is a
// pure function (`lib/language.js`), tested here without a WebView; the
// e2e spec `redesign-language` plays the real first launch pinned to `fr`.
import test from 'node:test';
import assert from 'node:assert/strict';
import { detectLanguage, LANGUAGES, DEFAULT_LANGUAGE } from '../apps/desktop/ui-v2/src/lib/language.js';

test('the default language is English', () => {
  assert.equal(DEFAULT_LANGUAGE, 'en');
  assert.deepEqual(LANGUAGES, ['en', 'fr']);
});

test('a French system speaks French, any other system speaks English', () => {
  assert.equal(detectLanguage('fr'), 'fr');
  assert.equal(detectLanguage('fr-CA'), 'fr');
  assert.equal(detectLanguage('FR-BE'), 'fr');
  assert.equal(detectLanguage('en-US'), 'en');
  assert.equal(detectLanguage('de-DE'), 'en');
  assert.equal(detectLanguage(''), 'en');
  assert.equal(detectLanguage(undefined), 'en');
  assert.equal(detectLanguage(null), 'en');
});
