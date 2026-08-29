// Spike S1 — variante C : repli de groupe À L'AFFICHAGE.
// La page est servie telle quelle (200 lignes, ordre de prod) ; le repli
// se fait en JS, comme le ferait Liste.svelte. On mesure :
//   - le temps de post-traitement par vol de 200 lignes (méd/p95) ;
//   - les rangées AFFICHABLES restantes par vol (moyenne, min, pires vols) ;
//   - combien de vols la rafale (600 messages / 12 h) consomme pour
//     produire SES rangées visibles.
// Usage : node affichage.mjs <rows.json>

import { readFileSync } from "node:fs";

const PAGE = 200;
const rows = JSON.parse(readFileSync(process.argv[2] ?? "rows.json", "utf8"));
console.log(`${rows.length} lignes servies (ordre de la liste), vols de ${PAGE}`);

// Post-traitement d'UN vol, avec l'état de session (groupes déjà vus).
// Première occurrence d'un expéditeur groupé -> 1 rangée de groupe ;
// occurrences suivantes -> rien. Le compte n du groupe ne peut être que
// « ce qui a été servi jusqu'ici » : le vrai total exigerait une
// requête à part (fait, pas opinion).
function replier(vol, dejaVus) {
  const visibles = [];
  for (const [sender, _unseen, groupe] of vol) {
    if (groupe === 1) {
      const n = dejaVus.get(sender) ?? 0;
      if (n === 0) visibles.push({ type: "groupe", sender });
      dejaVus.set(sender, n + 1);
    } else {
      visibles.push({ type: "fil", sender });
    }
  }
  return visibles;
}

const nVols = Math.ceil(rows.length / PAGE);
const affichables = [];
const temps = [];
{
  const dejaVus = new Map();
  for (let v = 0; v < nVols; v++) {
    const vol = rows.slice(v * PAGE, (v + 1) * PAGE);
    const t0 = performance.now();
    const visibles = replier(vol, dejaVus);
    temps.push(performance.now() - t0);
    affichables.push(visibles.length);
  }
}
temps.sort((a, b) => a - b);
const med = temps[Math.floor(temps.length / 2)];
const p95 = temps[Math.ceil(temps.length * 0.95) - 1];
console.log(`Post-traitement par vol : méd ${med.toFixed(3)} ms | p95 ${p95.toFixed(3)} ms`);

const somme = affichables.reduce((a, b) => a + b, 0);
console.log(`Rangées affichables par vol : moyenne ${(somme / nVols).toFixed(1)} / ${PAGE}, min ${Math.min(...affichables)}`);
const pires = affichables
  .map((n, i) => [i, n])
  .sort((a, b) => a[1] - b[1])
  .slice(0, 6);
console.log("Pires vols (index, affichables) :", pires.map(([i, n]) => `#${i}:${n}`).join("  "));

// La rafale : combien de vols pour écouler les 600 messages de bavard4,
// et le rendement de ces vols-là.
let premier = -1, dernier = -1, servisRafale = 0;
rows.forEach(([s], i) => {
  if (s === "bavard4@exemple.fr") {
    if (premier < 0) premier = i;
    dernier = i;
    servisRafale++;
  }
});
const volPremier = Math.floor(premier / PAGE);
const volDernier = Math.floor(dernier / PAGE);
console.log(`Rafale bavard4 : ${servisRafale} lignes servies, rangs ${premier}..${dernier}, vols #${volPremier}..#${volDernier} (${volDernier - volPremier + 1} vols traversés) -> 1 rangée affichée`);
const rendement = affichables.slice(volPremier, volDernier + 1);
console.log(`Affichables dans ces vols : [${rendement.join(", ")}]`);
