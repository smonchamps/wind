<script>
  // LE menu du produit (PLAN-AUDIT-V2 E11 — clôt D-47 : huit copies du
  // dessin et de la mécanique, trois ombres, trois z-index, un jeton
  // `--ombre` inexistant ; A8 tenu : `role="menu"` promettait un clavier
  // absent). Un seul composant : ancrage, flèches ↑/↓, Début/Fin, Échap
  // et Tab ferment, clic dehors ferme, le focus se pose sur le premier
  // item à l'ouverture et REVIENT au déclencheur à la fermeture. Le
  // parent ne fournit que les items (snippet) et l'état `ouvert`.
  let {
    ouvert = false,
    x = 0,
    y = 0,
    testid = 'menu',
    largeur = 240,
    // `absolu` : ancré sous son déclencheur (position:absolute dans un
    // parent relatif) au lieu de coordonnées fixes.
    absolu = false,
    onfermer = () => {},
    children,
  } = $props();

  let mailbox = $state(null);
  const items = () =>
    mailbox ? [...mailbox.querySelectorAll('[role^="menuitem"]:not([disabled])')] : [];

  $effect(() => {
    if (!ouvert) return;
    const declencheur = document.activeElement;
    // Après le rendu : le menu se BORNE à la fenêtre (sa vraie taille,
    // pas une constante recopiée sept fois — revue), puis le premier
    // item prend le focus.
    queueMicrotask(() => {
      if (mailbox && !absolu) {
        const r = mailbox.getBoundingClientRect();
        if (r.right > window.innerWidth - 8) mailbox.style.left = `${Math.max(8, window.innerWidth - r.width - 8)}px`;
        if (r.bottom > window.innerHeight - 8) mailbox.style.top = `${Math.max(8, window.innerHeight - r.height - 8)}px`;
      }
      items()[0]?.focus();
    });
    const clic = (e) => {
      // Le clic d'OUVERTURE arrive ici aussi (l'effet court pendant sa
      // propagation) : un clic sur le déclencheur n'est jamais « dehors »
      // — le parent décide de sa bascule.
      if (declencheur?.contains?.(e.target)) return;
      if (mailbox && !mailbox.contains(e.target)) onfermer();
    };
    const touche = (e) => {
      const liste = items();
      if (liste.length === 0) return;
      const i = liste.indexOf(document.activeElement);
      switch (e.key) {
        case 'Escape':
          e.preventDefault();
          onfermer();
          break;
        case 'Tab':
          onfermer();
          break;
        case 'ArrowDown':
          e.preventDefault();
          liste[(i + 1) % liste.length].focus();
          break;
        case 'ArrowUp':
          e.preventDefault();
          liste[(i - 1 + liste.length) % liste.length].focus();
          break;
        case 'Home':
          e.preventDefault();
          liste[0].focus();
          break;
        case 'End':
          e.preventDefault();
          liste[liste.length - 1].focus();
          break;
        default:
      }
    };
    window.addEventListener('click', clic);
    window.addEventListener('keydown', touche);
    return () => {
      window.removeEventListener('click', clic);
      window.removeEventListener('keydown', touche);
      // Le focus revient d'où il est parti — le déclencheur, s'il vit.
      if (declencheur?.isConnected && typeof declencheur.focus === 'function') declencheur.focus();
    };
  });
</script>

{#if ouvert}
  <div class="menu" class:absolu role="menu" data-testid={testid} bind:this={mailbox}
       style={absolu ? `min-width:${largeur}px` : `left:${x}px; top:${y}px; min-width:${largeur}px`}>
    {@render children()}
  </div>
{/if}

<style>
  /* Le dessin unique (famille D-47) : la carte flottante des gestes de
     la Liste — z-index 30 (au-dessus des barres collantes, sous rien
     d'autre : un menu est toujours l'objet le plus haut), l'ombre du
     jeton --shadow, le rayon des contrôles. */
  .menu {
    position:fixed; z-index:30; padding:6px; display:flex; flex-direction:column;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); box-shadow:var(--shadow);
  }
  .menu.absolu { position:absolute; top:calc(100% + 6px); left:0; }
  .menu :global(button[role^="menuitem"]) {
    display:flex; align-items:center; gap:10px; width:100%;
    height:32px; padding:0 8px; font-size:13px; color:var(--ink);
    background:none; border:1px solid transparent;
    border-radius:var(--r-controle); cursor:pointer; text-align:left;
    white-space:nowrap;
  }
  .menu :global(button[role^="menuitem"]:hover),
  .menu :global(button[role^="menuitem"]:focus-visible) { background:var(--hover); outline:none; }
  .menu :global(button[aria-checked="true"]) { font-weight:600; }
  .menu :global(.filet-menu), .menu :global(.filet) { height:1px; background:var(--border); margin:4px 0; }
  .menu :global(.titre-menu) {
    margin:4px 8px 2px; font-size:11px; font-weight:600; letter-spacing:.02em;
    text-transform:uppercase; color:var(--muted);
  }
</style>
