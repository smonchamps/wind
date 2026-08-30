// La hauteur d'un corps en iframe suit le CONTENU (terrain A47),
// jamais un gabarit fixe : l'iframe est same-origin SANS scripts
// (invariant S1) — le parent mesure le document assaini et pose la
// hauteur. Re-mesure au chargement (srcdoc posé, images accordées) et
// au changement de LARGEUR seulement (re-flow du texte) — jamais sur
// sa propre pose de hauteur, pour ne pas boucler l'observateur.
//
// Extraite de Fil.svelte à E5bis (le Kiosque en cartes affiche les
// mêmes corps) : UNE porte, jamais deux copies qui divergent.
export function corpsAuto(iframe) {
  let largeur = 0;
  const mesurer = () => {
    const doc = iframe.contentDocument;
    if (!doc?.documentElement) return;
    iframe.style.height = '0';
    iframe.style.height = `${doc.documentElement.scrollHeight}px`;
  };
  const surLoad = () => {
    largeur = iframe.offsetWidth;
    mesurer();
  };
  iframe.addEventListener('load', surLoad);
  const observateur = new ResizeObserver(() => {
    if (iframe.offsetWidth === largeur) return;
    largeur = iframe.offsetWidth;
    mesurer();
  });
  observateur.observe(iframe);
  return {
    destroy() {
      observateur.disconnect();
      iframe.removeEventListener('load', surLoad);
    },
  };
}
