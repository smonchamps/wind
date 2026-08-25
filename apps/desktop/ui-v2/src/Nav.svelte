<script>
  // Nav 248 px de l'écran 02 — le dessin des pistes (A29, amendé V4/
  // V14) : rangées 14 px à coin vif, item actif en teinte de sélection
  // bordée d'accent, compteur de non-lus en NOMBRE nu à l'accent (la
  // pastille pleine est morte). Les compteurs « héros / total » quittent
  // la nav (W2-D4) — la barre de statut dit les totaux. Les six dossiers
  // canoniques, puis « Boîtes » : Toutes les boîtes + un rang par compte
  // RÉEL — la fiction « Travail / Personnel » n'existe pas ; icône
  // `person` (décision D7), libellé = adresse. La boîte EN COURS prend
  // le dessin de la tuile d'événement (--tuile/--tuileInk, W2-D5).
  import Icone from './Icone.svelte';
  import { activation } from './lib/clavier.js';
  import { t } from './lib/texte.svelte.js';

  // R1 (PLAN-RETOURS-8, A74), amendé A82 : un compte peut porter un
  // repère (icône du jeu dédié + teinte du nuancier) — il remplace
  // `person` par le TRACÉ nu du glyphe à la teinte du compte, 16 px
  // comme les glyphes de dossier (la pastille pleine a quitté la nav :
  // la nav et la ligne portent exactement le même objet, D2). Sans
  // repère, le rendu D7 d'origine ne change pas.
  // PLAN-RETOURS-9 (D4) : le nom personnalisé REMPLACE l'adresse sur
  // la tuile — la nav dit l'identité choisie, pas la donnée technique.
  let { comptes = [], reperes = {}, noms = {}, categorie, compte, onchoisir = () => {} } = $props();

  const somme = (champ) => comptes.reduce((n, c) => n + c[champ], 0);

  // Le filtre de compte borne les compteurs des dossiers, comme au
  // prototype où changer de Boîte re-filtre la liste.
  const vue = $derived(
    compte === null ? null : comptes.find((c) => c.account_id === compte),
  );
  const de = (champ) => (vue ? vue[champ] : somme(champ));

  const dossiers = $derived([
    {
      id: 'reception', icone: 'inbox', libelle: t('boite.reception'),
      nonLus: de('reception_non_lues'),
    },
    { id: 'envoyes', icone: 'send', libelle: t('boite.envoyes') },
    { id: 'brouillons', icone: 'edit_note', libelle: t('boite.brouillons') },
    {
      id: 'indesirables', icone: 'report', libelle: t('boite.indesirables'),
      nonLus: de('indesirables_non_lus'),
    },
    { id: 'archives', icone: 'inventory_2', libelle: t('boite.archives') },
    { id: 'corbeille', icone: 'delete', libelle: t('boite.corbeille') },
  ]);

  // La tuile ne compte rien (A36, terrain E3) : la pastille de la
  // Réception dit déjà le non-lu — la tuile ne porte que l'identité.
  const boites = $derived([
    { id: null, icone: 'all_inbox', libelle: t('nav.toutes') },
    ...comptes.map((c) => ({
      id: c.account_id, icone: 'person', libelle: noms[c.account_id] ?? c.email,
      repere: reperes[c.account_id] ?? null,
    })),
  ]);
</script>

<nav aria-label={t('nav.aria')} data-testid="nav">
  {#each dossiers as d (d.id)}
    <div class="rang" class:actif={categorie === d.id}
         data-testid="nav-dossier" data-categorie={d.id}
         role="button" tabindex="0" aria-current={categorie === d.id}
         onclick={() => onchoisir({ categorie: d.id })}
         onkeydown={activation(() => onchoisir({ categorie: d.id }))}>
      <span class="icone" aria-hidden="true"><Icone nom={d.icone} /></span>
      <span class="libelle">{d.libelle}</span>
      {#if d.nonLus > 0}
        <span class="pastille">{d.nonLus}</span>
      {/if}
    </div>
  {/each}

  <div class="boites">
    <p class="titre">{t('nav.boites')}</p>
    {#each boites as b (b.id)}
      {#if compte === b.id}
        <!-- La boîte en cours : la tuile d'événement (A29, W2-D5),
             l'adresse seule — sans compteur (A36). -->
        <div class="tuile" data-testid="nav-boite"
             role="button" tabindex="0" aria-current="true"
             onclick={() => onchoisir({ compte: b.id })}
             onkeydown={activation(() => onchoisir({ compte: b.id }))}>
          {#if b.repere}
            <span class="repere-nu" data-testid="nav-repere"
                  data-teinte={b.repere.teinte} aria-hidden="true"><Icone nom={b.repere.icone} taille={16} /></span>
          {:else}
            <span class="icone-tuile" aria-hidden="true"><Icone nom={b.icone} /></span>
          {/if}
          <span class="titre-tuile">{b.libelle}</span>
        </div>
      {:else}
        <div class="rang" data-testid="nav-boite"
             role="button" tabindex="0" aria-current="false"
             onclick={() => onchoisir({ compte: b.id })}
             onkeydown={activation(() => onchoisir({ compte: b.id }))}>
          {#if b.repere}
            <span class="repere-nu" data-testid="nav-repere"
                  data-teinte={b.repere.teinte} aria-hidden="true"><Icone nom={b.repere.icone} taille={16} /></span>
          {:else}
            <span class="icone" aria-hidden="true"><Icone nom={b.icone} /></span>
          {/if}
          <span class="libelle">{b.libelle}</span>
        </div>
      {/if}
    {/each}
  </div>
</nav>

<style>
  nav {
    background:var(--bg); border-right:1px solid var(--border);
    padding:20px 12px; display:flex; flex-direction:column; gap:2px;
    min-height:0; overflow:auto;
  }
  .rang {
    display:flex; align-items:center; gap:10px; flex:none;
    padding:8px 10px; border-radius:var(--r-controle); cursor:pointer;
    border:1px solid transparent;
  }
  .rang:hover { background:var(--hover); }
  .rang.actif { background:var(--sel); border-color:var(--accent); }
  .icone { color:var(--muted); }
  .actif .icone { color:var(--accent); }
  .libelle {
    font-size:14px; color:var(--ink2); flex:1; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .actif .libelle { font-weight:600; color:var(--ink); }
  /* V4 — la pilule pleine est morte : le compteur est un NOMBRE nu,
     chiffres tabulaires à l'accent (paire accent/bg mesurée en TEXTE). */
  .pastille {
    flex:none; font-size:12px; font-weight:600; color:var(--accent);
    font-variant-numeric:tabular-nums;
  }
  .boites {
    margin-top:auto; padding-top:16px; border-top:1px solid var(--border);
    display:flex; flex-direction:column; gap:6px;
  }
  .titre {
    margin:0 0 4px; padding:0 10px; font-size:11px; letter-spacing:.1em;
    text-transform:uppercase; color:var(--muted); font-weight:600;
  }
  .tuile {
    display:flex; align-items:center; gap:10px; flex:none;
    padding:9px 12px; border-radius:var(--r-controle); cursor:pointer;
    background:var(--tuile); color:var(--tuileInk);
    border:1px solid var(--border);
  }
  .icone-tuile { color:var(--tuileInk); }
  .titre-tuile {
    font-size:13px; font-weight:600; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
</style>
