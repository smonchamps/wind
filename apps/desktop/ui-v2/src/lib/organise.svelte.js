// Le Mode organisé (PLAN-MODE-ORGANISE E1, décision D2 amendée du
// 2026-08-29) : l'état vit en `prefs` SQLite, PAS en localStorage —
// le CŒUR doit le lire (les règles du Non de l'étape E3 s'éteignent
// avec le mode), l'UI ne fait que le refléter. La borne de rétention
// (époque de première activation, D3 « arrivées seules ») s'écrit côté
// Rust dans le même geste — jamais ici.
import { appel } from './transport.js';

const etat = $state({ actif: false });

// Revue E1 : la restauration part sans await (leçon PLAN-DEMARRAGE —
// rien ne précède la première page de la liste) ; si l'utilisateur
// bascule AVANT qu'elle ne résolve, sa lecture périmée ne doit jamais
// écraser le geste frais.
let bascule = false;
let enVol = false;

export function modeOrganise() {
  return etat.actif;
}

// Lu une fois au démarrage, APRÈS la première page de la liste (l'App
// décide du moment — leçon PLAN-DEMARRAGE E2). Un échec laisse le
// mode éteint : le classique est le défaut.
export async function restaurerModeOrganise() {
  try {
    const lu = Boolean(await appel('mode_organise_get'));
    if (!bascule) etat.actif = lu;
  } catch {
    /* le classique est le défaut, rien à refléter */
  }
  return etat.actif;
}

// La bascule ÉCRIT d'abord, reflète ensuite — un échec de commande ne
// laisse jamais l'UI dire un mode que la base n'a pas. Un second clic
// pendant le vol est ignoré (sinon les deux calculent la même cible
// et la bascule « colle »).
export async function basculerModeOrganise() {
  if (enVol) return etat.actif;
  enVol = true;
  bascule = true;
  try {
    const cible = !etat.actif;
    await appel('mode_organise_set', { actif: cible });
    etat.actif = cible;
    return cible;
  } finally {
    enVol = false;
  }
}
