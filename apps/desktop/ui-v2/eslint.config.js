// E1c candidate (PLAN-ENGLISH-SWITCH): the one rule the switch needs —
// an identifier used but defined nowhere. JS has no compiler; a renamed
// function whose caller was missed only fails at run time, on the screen
// the e2e happen to open. `no-undef` finds it in seconds.
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';

const runes = Object.fromEntries(
  ['$state', '$derived', '$effect', '$props', '$bindable', '$inspect', '$host'].map((r) => [r, 'readonly']),
);

export default [
  ...svelte.configs['flat/base'],
  {
    files: ['src/**/*.js', 'src/**/*.svelte'],
    languageOptions: {
      ecmaVersion: 'latest',
      sourceType: 'module',
      globals: { ...globals.browser, ...runes },
    },
    rules: { 'no-undef': 'error' },
  },
];
