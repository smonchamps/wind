// R2 (PLAN-RETOURS-8, A75) — the first-launch flow is only played
// ONCE. The markers live in localStorage (V-D4: a pure UI preference,
// the shell has nothing to read from it — the wind-theme / wind-volets
// pattern). Two keys: `fait` (Finish clicked, or an existing
// installation deemed onboarded) and `commence` (the flow has been
// shown) — it is this one that distinguishes an update (accounts
// present, never a flow → deemed done) from a flow ABANDONED
// mid-course (an account added at step 1, the app quit before Finish
// → it resumes on the next launch).
//
// e2e seam `__e2eOnboarding` (defensive read, __e2eLinks pattern):
// it lives HERE, at the persistence boundary — never in App.svelte's
// product decision. Under the seam, nothing is “done” and nothing is
// written: a seeded fixture replays the whole flow without polluting
// the profile.
const DONE_KEY = 'wind-accueil-fait';
const STARTED_KEY = 'wind-accueil-commence';

const forceE2e = () => globalThis.window?.__e2eOnboarding === true;

export function onboardingDone() {
  if (forceE2e()) return false;
  try {
    return localStorage.getItem(DONE_KEY) === '1';
  } catch {
    // Storage unavailable: deemed done — a flow that came back on
    // EVERY launch would be worse than a missed flow (and
    // `markOnboardingDone` could not turn it off).
    return true;
  }
}

export function markOnboardingDone() {
  if (forceE2e()) return;
  try {
    localStorage.setItem(DONE_KEY, '1');
  } catch { /* storage unavailable: accueilFait() already answers “done” */ }
}

export function onboardingStarted() {
  if (forceE2e()) return false;
  try {
    return localStorage.getItem(STARTED_KEY) === '1';
  } catch {
    return false;
  }
}

export function markOnboardingStarted() {
  if (forceE2e()) return;
  try {
    localStorage.setItem(STARTED_KEY, '1');
  } catch { /* storage unavailable: the resume will not survive, without breakage */ }
}
