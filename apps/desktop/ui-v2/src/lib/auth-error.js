// Backlog 86 (E6): the ONE OAuth failure the user can fix — partial
// consent — crosses the IPC with the stable code `missing_mail_scope`
// (fault.rs, the `app_location_readonly` convention: the UI branches
// on the code, never on the wording). The caller names the provider —
// the surface always knows which consent it just ran.
import { t } from './text.svelte.js';

export function connectionError(err, fallbackKey, provider) {
  if (err?.code === 'missing_mail_scope') {
    return t('error.missingScope', { provider });
  }
  return t(fallbackKey, { err });
}
