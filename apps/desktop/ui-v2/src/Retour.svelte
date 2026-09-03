<script>
  // Le formulaire de retour bêta (PLAN-RETOURS-11 R3, terrain du
  // 2026-08-28) : un champ, un envoi. Le message part par LA boîte
  // d'envoi (`queue_send`) — la règle d'or « jamais d'envoi perdu »,
  // le filet hors-ligne et la barre d'état valent ici gratuitement,
  // aucun chemin d'envoi neuf. L'adresse des retours est une constante
  // produit (décision CE D7 / terrain bêta) ; l'expéditeur est le
  // premier compte du poste — le bouton n'existe pas sans compte
  // (App.svelte le garde).
  import Icone from './Icone.svelte';
  import { appel } from './lib/transport.js';
  import { t } from './lib/texte.svelte.js';

  const ADRESSE_RETOURS = 'feedback-wind@fcts.io';

  let { comptes = [], onflash = () => {} } = $props();

  let visible = $state(false);
  let texte = $state('');
  let envoiEnCours = $state(false);
  let champ = $state(null);

  export function ouvrir() {
    texte = '';
    visible = true;
  }
  export function estOuverte() {
    return visible;
  }
  export function fermer() {
    visible = false;
  }

  $effect(() => {
    if (visible) champ?.focus();
  });

  async function envoyer() {
    const account = comptes[0];
    if (envoiEnCours || !texte.trim() || !account) return;
    envoiEnCours = true;
    try {
      // La version voyage dans le SUJET — le dépouillement des retours
      // en a besoin, et le testeur n'a rien à recopier. Si elle
      // manque, le retour part quand même.
      let version = '';
      try {
        version = await appel('app_version');
      } catch {
        version = '';
      }
      await appel('queue_send', {
        accountId: account.account_id,
        to: ADRESSE_RETOURS,
        cc: '',
        bcc: '',
        subject: version ? `Retour Wind ${version}` : 'Retour Wind',
        body: texte.trim(),
        bodyHtml: null,
        replyToMailbox: null,
        replyToUid: null,
        draftId: null,
        important: false,
        sendAtEpoch: null,
      });
    } catch (err) {
      onflash(t('erreur.envoi', { err }));
      return;
    } finally {
      envoiEnCours = false;
    }
    visible = false;
    onflash(t('retour.merci'));
    // Le départ IMMÉDIAT, comme au composeur (terrain du 2026-08-28 :
    // sans lui, le retour attendait la prochaine vidange — cycle ou
    // synchro manuelle). En arrière-plan ; hors ligne, la file attend
    // et la barre d'état le dit — le chemin d'envoi normal.
    appel('flush_outbox').catch((err) => console.error('flush_outbox :', err));
  }
</script>

{#if visible}
  <div class="scrim" role="presentation"
       onclick={(e) => { if (e.target === e.currentTarget) fermer(); }}>
    <!-- tabindex -1 : le dialog est focalisable par programme (a11y du
         rôle) ; le focus réel part au champ dès l'ouverture. -->
    <div class="carte" role="dialog" aria-modal="true" tabindex="-1"
         aria-label={t('retour.titre')} data-testid="retour-carte"
         onkeydown={(e) => { if (e.key === 'Escape') fermer(); }}>
      <div class="tete">
        <Icone name="feedback" />
        <span class="titre">{t('retour.titre')}</span>
        <button type="button" class="fermer" aria-label={t('action.fermer')}
                onclick={fermer}><Icone name="close" /></button>
      </div>
      <p class="sous">{t('retour.sous')}</p>
      <textarea bind:this={champ} bind:value={texte} rows="6"
                data-testid="retour-texte"
                placeholder={t('retour.placeholder')}
                aria-label={t('retour.titre')}></textarea>
      <div class="pied">
        <button type="button" class="secondaire" onclick={fermer}>
          {t('action.annuler')}</button>
        <!-- « Envoyer » ABSENT tant que le champ est vide — jamais
             grisé (la règle du parcours d'accueil, RETOURS-8 D4). -->
        {#if texte.trim() && !envoiEnCours}
          <button type="button" class="principal" data-testid="retour-envoyer"
                  onclick={envoyer}>{t('action.envoyer')}</button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  /* La surimpression aux jetons de la carte des Réglages, en petit. */
  .scrim {
    position:absolute; inset:0; background:var(--scrim); z-index:2;
    display:flex; align-items:center; justify-content:center; padding:36px;
  }
  .carte {
    width:520px; max-width:100%; background:var(--surface);
    border:1px solid var(--border);
    border-radius:var(--r-surface); box-shadow:var(--shadow);
    display:flex; flex-direction:column; gap:10px; padding:18px 22px 16px;
  }
  .tete { display:flex; align-items:center; gap:10px; color:var(--ink); }
  .tete :global(.ic) { color:var(--ink2); }
  .titre { font-size:15px; font-weight:600; flex:1; }
  .fermer {
    height:32px; width:32px; padding:0; display:inline-flex;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); cursor:pointer;
  }
  .fermer:hover { background:var(--sel); }
  .sous { margin:0; font-size:12px; line-height:1.5; color:var(--muted); }
  textarea {
    resize:vertical; min-height:110px; padding:10px 12px;
    font:inherit; font-size:13px; color:var(--ink);
    background:var(--bg); border:1px solid var(--border);
    border-radius:var(--r-controle);
  }
  textarea:focus-visible { outline:2px solid var(--accent); outline-offset:2px; }
  .pied { display:flex; justify-content:flex-end; gap:8px; }
  .secondaire, .principal {
    height:32px; padding:0 14px; font-size:13px; cursor:pointer;
    border-radius:var(--r-controle);
  }
  .secondaire {
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border);
  }
  .secondaire:hover { background:var(--sel); }
  .principal {
    color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent); font-weight:600;
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }
</style>
