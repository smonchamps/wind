// Thèmes « Elements » (V7 — PLAN-ELEMENTS) : deux thèmes, et deux
// seulement — `elements` (défaut, aucun attribut posé) et sa
// déclinaison sombre `elements-nuit`. La table Wada de 28 thèmes
// (A42) est retirée ; la mécanique du suivi OS est conservée : quand
// le suivi de l'OS sombre est actif et que l'OS est en sombre, la
// déclinaison -nuit du thème CHOISI s'affiche — le choix persisté
// reste le thème de base, le suffixe est un état dérivé, jamais
// enregistré. Un thème -nuit choisi à la main reste en paix.

const CLE = 'wind-theme';
const CLE_AUTO = 'wind-theme-auto';

// Recopie des clés Discovery d'avant la bascule (PLAN-WIND E3) : le
// choix survit au renommage, les anciennes clés sont retirées. Le
// profil WebView2 est déménagé tel quel par l'application — les
// anciennes clés sont donc bien là au premier lancement Wind.
try {
  for (const [ancienne, neuve] of [['discovery-theme', CLE], ['discovery-theme-auto', CLE_AUTO]]) {
    const valeur = localStorage.getItem(ancienne);
    if (valeur !== null) {
      if (localStorage.getItem(neuve) === null) localStorage.setItem(neuve, valeur);
      localStorage.removeItem(ancienne);
    }
  }
  // V7 : la table Wada est retirée — la POLARITÉ est le seul choix qui
  // survive, et elle est ÉCRITE (le motif d'A42, qui migrait « nuit »
  // vers nature-nuit, rejoué sur la table entière) : tout ancien choix
  // sombre (`nuit` d'avant A42, ou un `<wada>-nuit`) devient
  // `elements-nuit`. Les anciens choix CLAIRS retombent sur le défaut
  // par le garde-fou de themeActuel(), silencieusement — comme les
  // cinq thèmes retirés d'A42 avant eux.
  const choix = localStorage.getItem(CLE);
  if (choix !== null && !['elements', 'elements-nuit'].includes(choix)
      && (choix === 'nuit' || choix.endsWith('-nuit'))) {
    localStorage.setItem(CLE, 'elements-nuit');
  }
} catch { /* stockage indisponible : rien à migrer */ }

// Les fiches du sélecteur (Réglages, accueil) — pastilles VERBATIM de
// la table du contrat, dans l'ordre accent, fond, filet, surface,
// encre (`panel` est mort — V3 ; le filet entre à sa place : depuis V3
// c'est LUI qui dessine la séparation, les vignettes le montrent).
// Les mêmes valeurs vivent en jetons dans systeme.css : les pastilles
// doivent montrer chaque thème SANS l'appliquer, d'où les hex répétés
// ici — la gate coherence-systeme.mjs les tient égales aux jetons
// livrés. Libellés et descriptions vivent au catalogue
// (`theme.<id>.nom` / `theme.<id>.desc` — PLAN-LANGUES, A15).
export const FICHES = [
  { id: 'elements', pastilles: ['#1A7A7A', '#F3F2EE', '#CBC8BB', '#FFFFFF', '#191D1E'] },
  { id: 'elements-nuit', pastilles: ['#3FA39C', '#0D100F', '#333B3A', '#171B1A', '#ECEDEA'] },
];

// La liste des identifiants se DÉRIVE des fiches : une seule table à
// maintenir — une fiche sans thème (ou l'inverse) est impossible par
// construction, et la gate coherence-systeme.mjs tient les pastilles
// égales aux jetons livrés.
export const THEMES = FICHES.map((f) => f.id);

export function themeActuel() {
  let nom = 'elements';
  try { nom = localStorage.getItem(CLE) || 'elements'; } catch { /* stockage indisponible : défaut */ }
  return THEMES.includes(nom) ? nom : 'elements';
}

// Constat terrain A42 (2026-08-16, sondes au banc) : dans le WebView2
// de Tauri/wry, prefers-color-scheme ne suit PAS l'OS — jamais sombre,
// zéro événement, même sous une vraie bascule (registre + diffusion
// WM_SETTINGCHANGE). La source est l'API fenêtre Tauri (theme() +
// onThemeChanged), qui a tiré dans les deux sens à chaque bascule ;
// matchMedia reste le repli hors Tauri et la poignée du banc e2e
// (emulateMedia). Les DEUX canaux écrivent le même état et le dernier
// signal gagne — jamais un OU permanent : machine hôte en sombre, un
// OU rendrait emulateMedia('light') à jamais perdant (constaté au banc,
// D6 rouge à la première version du remède).
let sombreSignale = null;

function osSombre() {
  return sombreSignale
    ?? (globalThis.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false);
}

function poser(nom) {
  if (nom === 'elements') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = nom;
  // A44 : le `color-scheme` des barres natives ne se pose PAS ici —
  // il vit en CSS, à côté des jetons (`:root[data-theme$="-nuit"]`,
  // systeme.css) : tout chemin qui pose data-theme l'obtient, sans JS.
  // Revue A42 : la coche de Réglages suit la fiche AFFICHÉE — le
  // signal dit « le thème posé vient de changer », quel qu'en soit le
  // chemin (choix, bascule du suivi, événement OS en cours de session).
  document.dispatchEvent(new CustomEvent('wind:theme-affiche'));
}

// Le thème POSÉ sur <html> — l'état dérivé, jamais persisté (A42).
// C'est lui que la coche de Réglages désigne (revue A42) : sous suivi
// OS + OS sombre, l'œil voit la déclinaison -nuit, la coche aussi.
export function themeAffiche() {
  return document.documentElement.dataset.theme ?? 'elements';
}

// R3 (PLAN-RETOURS-4, D3, 2026-08-18) : `paletteLecture()` est RETIRÉE.
// Le corps d'un message s'affiche désormais sur dalle claire dans tous
// les thèmes (le cœur bake `Palette::default` — voir `message_body`) :
// la dalle sombre d'A42 rendait illisible le texte à couleurs
// d'expéditeur. Le front n'a donc plus de palette à transmettre.

// L'unique endroit qui décide du thème AFFICHÉ : quand le suivi de
// l'OS est actif et que l'OS est sombre, la déclinaison -nuit du thème
// choisi (A42). L'appartenance à THEMES est la garde unique : elle
// laisse en paix un thème déjà -nuit (« elements-nuit-nuit » n'existe
// pas) et refuse de poser un attribut orphelin qu'aucun bloc CSS ne
// servirait — le choix s'affiche tel quel plutôt qu'en palette défaut.
function refleter() {
  const choisi = themeActuel();
  const nuit = `${choisi}-nuit`;
  poser(suiviOs() && osSombre() && THEMES.includes(nuit) ? nuit : choisi);
}

export function suiviOs() {
  try { return localStorage.getItem(CLE_AUTO) === '1'; } catch { return false; }
}

export function appliquerSuiviOs(actif) {
  try { localStorage.setItem(CLE_AUTO, actif ? '1' : '0'); } catch { /* le choix ne survivra pas, rien d'autre à faire */ }
  refleter();
}

export function appliquerTheme(nom) {
  if (!THEMES.includes(nom)) return;
  try { localStorage.setItem(CLE, nom); } catch { /* stockage indisponible : le choix ne survivra pas, rien d'autre à faire */ }
  refleter();
}

export function restaurerTheme() {
  // L'OS peut basculer en cours de session (mode nuit planifié) : le
  // reflet suit sans redémarrage, par le canal Tauri (le seul vivant
  // en production — constat terrain A42) ET par matchMedia (repli hors
  // Tauri, poignée e2e). Les rejets se taisent : hors Tauri la fenêtre
  // n'existe pas, le repli fait foi.
  const fenetre = globalThis.window?.__TAURI__?.window?.getCurrentWindow?.();
  if (fenetre) {
    fenetre.theme()
      .then((t) => {
        // L'état initial ne bat jamais un signal déjà arrivé.
        if (sombreSignale === null) {
          sombreSignale = t === 'dark';
          refleter();
        }
      })
      .catch(() => { /* thème illisible : le repli matchMedia fait foi */ });
    fenetre.onThemeChanged(({ payload }) => {
      sombreSignale = payload === 'dark';
      refleter();
    }).catch(() => { /* écoute refusée : le repli matchMedia fait foi */ });
  }
  refleter();
  globalThis.matchMedia?.('(prefers-color-scheme: dark)')
    .addEventListener?.('change', (e) => {
      sombreSignale = e.matches;
      refleter();
    });
  return themeActuel();
}
