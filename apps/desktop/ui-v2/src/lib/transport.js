// Tauri transport: successful values pass through unchanged. Ordinary failures
// remain strings; typed size refusals become Errors with a stable code and limit.

// The e2e seams are COMPILED OUT of a release build (PLAN-AUDIT-V3
// E7, D-52 item 8): vite replaces `import.meta.env.VITE_E2E` with a
// literal at build time, and the minifier drops every dead branch —
// the release bundle carries neither the reads nor the `__e2e*`
// property names (make-release asserts their absence). The e2e build
// (VITE_E2E=1, set by e2e/rebuild-v2.mjs) keeps the exact historical
// paths.
const E2E = import.meta.env.VITE_E2E === '1';

const invoke = globalThis.window?.__TAURI__?.core?.invoke;

const brut = invoke
  ? (command, args) => invoke(command, args)
  : (command) => Promise.reject(
      `transport unavailable: ${command} (outside Tauri, no remote implementation delivered)`);

// e2e seam (PLAN-REACTIVITE E1): a promise set in
// `window.__e2eHold` HOLDS every call to the core until it
// resolves — the assertion “a reload never shows the wait” must
// observe the screen WHILE a re-serve is in flight. Outside e2e the
// variable does not exist: the path is identical to before.
//
// e2e seam (PLAN-DEFILEMENT-PROFOND E1): an array set in
// `window.__e2eLog` receives a record {command, start, arrival}
// per call — the assertion “never more than N pages in flight” counts
// the flights open at each instant. Same rule: outside e2e, nothing.
// e2e seam (PLAN-RETOURS-12 R1): OAuth consent cannot be driven by
// Playwright — addresses set in `window.__e2eAdd` make the addition
// succeed without a browser, and `connect_accounts`'s report carries
// them as connected, mirroring the session that the core sets on a
// real addition (add_oauth_account). Outside e2e the variable does not
// exist. Returns a LAUNCHER (not a promise): the `__e2eHold` hold
// also applies to these flights — the seam modulates the transport,
// never the order of things (review). The returns are minimal: no one
// reads the addition report, and only `email` is consumed from the
// connection report — a richer contract would be a test's lie.
const fakeAdd = (command, args) => {
  const add = E2E ? globalThis.window?.__e2eAdd : undefined;
  if (!Array.isArray(add)) return null;
  if (command === 'add_account' || command === 'add_microsoft_account') {
    return () => Promise.resolve();
  }
  if (command === 'ui_state') {
    return () => brut(command, args).then((state) => ({
      ...state, connected: [...state.connected, ...add],
    }));
  }
  if (command === 'connect_accounts') {
    return () => brut(command, args).then((report) => ({
      ...report,
      accounts: [...report.accounts, ...add.map((email) => ({ email }))],
    }));
  }
  return null;
};

// e2e seam (PLAN-AUDIT-V2 E10): an array of command names set
// in `window.__e2eFailure` makes the next call of each one FAIL (once)
// — the only way to play “the core has not answered” on a fixture.
// Outside e2e the variable does not exist: identical path.
const fakeFailure = (command) => {
  const failure = E2E ? globalThis.window?.__e2eFailure : undefined;
  if (!Array.isArray(failure)) return null;
  const i = failure.indexOf(command);
  if (i === -1) return null;
  failure.splice(i, 1);
  return () => Promise.reject(new Error(`e2e failure: ${command}`));
};

export const call = (command, args) => {
  const consent = E2E && ['oauth_begin', 'oauth_status', 'oauth_cancel', 'add_account',
    'add_microsoft_account', 'reconnect_account'].includes(command)
    ? globalThis.window?.__e2eOAuth : undefined;
  // Exercise partial sync reports without connecting the isolated E2E account.
  const summary = E2E && command === 'sync_inbox_light' ? globalThis.window?.__e2eSyncSummary : undefined;
  const backfill = E2E && ['backfill_status', 'backfill_bodies'].includes(command) ? globalThis.window?.__e2eBackfill : undefined;
  const launch = (consent ? () => consent(command, args) : null) ?? (backfill ? () => Promise.resolve(backfill(command)) : null) ?? fakeFailure(command) ?? fakeAdd(command, args)
    ?? (summary ? () => Promise.resolve(summary) : () => brut(command, args));
  const heldCommands = E2E ? globalThis.window?.__e2eHoldCommands : undefined;
  const hold = E2E && (!heldCommands || heldCommands.includes(command))
    ? globalThis.window?.__e2eHold : undefined;
  const rawFlight = hold ? hold.then(launch) : launch();
  // Hold a real cached-body response after IPC to test revocation races.
  const afterBody = E2E && command === 'message_body' ? globalThis.window?.__e2eAfterBody : undefined;
  const injectIssues = E2E && ['ui_state', 'sync_progress'].includes(command) ? globalThis.window?.__e2eSyncIssues : undefined;
  const withIssues = injectIssues ? rawFlight.then((value) => command === 'ui_state'
    ? { ...value, sync: { ...value.sync, issues: injectIssues } } : { ...value, issues: injectIssues }) : rawFlight;
  const flight = afterBody ? withIssues.then(afterBody) : withIssues;
  const log = E2E ? globalThis.window?.__e2eLog : undefined;
  if (log) {
    const poll = { command, start: performance.now(), arrival: null };
    if (command === 'add_generic_account') poll.username = args.input.username;
    if (command === 'repair_generic_account') {
      poll.username = args.input.username;
      poll.accountId = args.accountId;
    }
    log.push(poll);
    const settle = () => {
      poll.arrival = performance.now();
    };
    flight.then(settle, settle);
  }
  return flight.catch((err) => {
    if (err?.code === 'remote_message_too_large') {
      const refusal = new Error(err.message);
      refusal.code = err.code;
      refusal.limit = err.limit;
      throw refusal;
    }
    throw err;
  });
};

// The native file picker (dialog plugin), over the SAME invoke
// channel as the rest — no global API to inject, a single
// permission (dialog:allow-open). Returns a list of paths, empty if
// the user cancels.
//
// e2e seam (PLAN-PIECES-JOINTES §7): the native dialog box
// cannot be driven by Playwright — the suite drops its fixture
// paths in `window.__e2eAttachments` and the picker never opens;
// the rest of the path (attach_files → chips → send) is the real one.
export const chooseFiles = async () => {
  const injectes = E2E ? globalThis.window?.__e2eAttachments : undefined;
  if (injectes !== undefined) return Array.isArray(injectes) ? injectes : [];
  const choice = await call('plugin:dialog|open', { options: { multiple: true } });
  if (!choice) return [];
  return Array.isArray(choice) ? choice : [choice];
};

// The native “Save as” dialog (dialog plugin), to download a
// received attachment (R1/PLAN-RETOURS-4). Same invoke channel as the
// rest; `defaultPath` prefills folder + name (Downloads + sanitized
// name, supplied by the core). Returns the chosen path, or null if
// the user cancels.
//
// e2e seam (symmetric to `chooseFiles`): a path set in
// `window.__e2eDestination` is returned as is, the native dialog —
// not drivable by Playwright — never opens.
export const chooseDestination = async (defaultPath) => {
  const injecte = E2E ? globalThis.window?.__e2eDestination : undefined;
  if (injecte !== undefined) return injecte || null;
  const choice = await call('plugin:dialog|save', { options: { defaultPath: defaultPath } });
  return choice || null;
};
