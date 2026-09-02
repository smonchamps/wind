<script>
  // Le FIL en cartes — l'objet unique de lecture (UI v3, verdict CE du
  // 2026-08-16, décision D4 ; dessin au trait de la maquette depuis le
  // terrain A45) : titre, puces d'inventaire, boutons nus à droite,
  // cartes aux avatars — repliées une ligne, dépliées en carte pleine
  // (« Nom <adresse> » / « À : … », heure longue — A92) —, fichiers joints,
  // brouillon du fil, barre des cinq gestes directs (D5 — jamais de
  // menu « Plus », exception b des annotations ; le « ⋯ » par message
  // de la maquette attend ses actions). Le volet de lecture
  // et l'écran 03 le montent TEL QUEL : deux cadres, un objet — l'état
  // vit dans lib/fil.svelte.js et survit au changement de cadre.
  //
  // Invariant S1 : une iframe sandbox `allow-same-origin` SANS
  // allow-scripts par message déplié, corps servi par le cœur,
  // liens interceptés (lib/liens.js).
  import Icone from './Icone.svelte';
  import BarreFil from './BarreFil.svelte';
  import {
    fil,
    cleMsg,
    basculerMessage,
    toutDeplier,
    toutReplier,
    afficherImages,
    toujoursAfficherImages,
    estEcho,
    cacheNoms,
    reessayer,
  } from './lib/fil.svelte.js';
  import { appel, choisirDestination } from './lib/transport.js';
  import { brancherLiens } from './lib/liens.js';
  import {
    tuileInvitation, quandInvitation, kickerInvitation, statutInvitation,
    ligneRepondant, lieuOrganisateur,
  } from './lib/invitation.js';
  import { quand, quandLong } from './lib/quand.js';
  import { blocBoite } from './lib/boite.js';
  import { initiales } from './lib/initiales.js';
  import { activation } from './lib/clavier.js';
  import { t } from './lib/texte.svelte.js';
  import { corpsAuto } from './lib/corps.js';

  let {
    brouillons = [],
    // A80/D5 : le bloc de boîte se répète au fil, derrière le nom de
    // l'expéditeur — carte dépliée ET rangée repliée. `comptes` sert la
    // seule garde D7 qui vaille ici : y a-t-il plus d'un compte ?
    reperes = {},
    noms = {},
    comptes = [],
    // La vue courante mélange-t-elle les comptes ? (App le sait seule :
    // elle tient le compte choisi ET l'état de la recherche.)
    melange = false,
    onreprendre = () => {},
    onarchiver = () => {},
    onsupprimer = () => {},
    onrepondre = () => {},
    onrepondretous = () => {},
    ontransferer = () => {},
    // R2 (PLAN-RETOURS-3, D2) : signaler un fil comme indésirable, ou le
    // ramener en Réception. Le geste vit dans la barre du fil, par fil ;
    // `estIndesirable` bascule le libellé selon la vue courante.
    onspam = () => {},
    onnonspam = () => {},
    estIndesirable = false,
    onflash = () => {},
    // Le cadre volet passe le geste d'agrandissement ; l'écran 03, non.
    onagrandir = null,
    // R4 (PLAN-RETOURS-7, D3/D4) : « Épingler » vit dans la barre du
    // fil, offert par la seule Réception (les épingles n'apparaissent
    // qu'en tête de la Réception — D4) ; l'App décide.
    epinglable = false,
    onepingler = () => {},
    // PLAN-MODE-ORGANISE E1 : « Déplacer vers… » — l'expéditeur ENTIER
    // change de destination (Réception / Kiosque / Registre), ses
    // messages suivent par construction (la requête lit le routage).
    // Offert par le seul mode organisé ; l'App décide.
    organise = false,
    ondeplacer = () => {},
    oncote = () => {},
  } = $props();

  // Le menu de « Déplacer vers… » — fermé par le choix, un second
  // clic, ou un CHANGEMENT de fil (revue E1 : le composant survit au
  // changement de ligne — sans ce reflet, le menu du fil A resterait
  // ouvert au-dessus du fil B et un clic distrait routerait B).

  // Depuis R4 (PLAN-RETOURS-3, D4) Répondre / Répondre à tous /
  // Transférer visent CHAQUE message (barre par message) — plus de
  // `cible()` unique au fil. Le raccourci clavier `r`, lui, reste sur la
  // sélection de liste (App.svelte), inchangé.
  //
  // Le compte de pièces FRAIS (après-scan, terrain CE 2026-08-14) : la
  // ligne porte celui d'AVANT l'ouverture — le store retient celui de
  // message_body dès que le corps est servi.
  const nbPiecesDe = (m) => fil.nbPieces[cleMsg(m)] ?? m.attachment_count;

  // A80/D5 : le compte se lit sur le MESSAGE (m.account_id — l'identité
  // canonique, invariant 2 du STANDARD) ; tous les messages d'un fil
  // viennent de la même boîte, l'adresse de repli est celle du fil
  // ouvert. Graisse normale : c'est le nom qui porte l'autorité.
  // LA règle du bloc (lib/boite.js), partagée avec la liste. Le compte
  // se lit sur le MESSAGE (identité canonique, invariant 2 du
  // STANDARD) ; l'adresse de repli est celle du fil ouvert.
  //
  // Verdict terrain du 2026-08-25 (point 12) : le volet suit la MÊME
  // garde de vue que la liste — dans la vue d'un seul compte, la liste
  // se taisait et le volet parlait encore. `melange` descend de l'App,
  // qui seule connaît la vue courante et l'état de la recherche.
  const boiteDe = (m) =>
    !melange
      ? null
      : blocBoite({
        accountId: m.account_id,
        adresse: fil.ligne?.account_email ?? '',
        reperes,
        noms,
        comptes,
      });

  // RETOURS-14 R4 (D5) : le signe du « fil mêlé » — la règle d'or
  // laisse un fil entier en Réception dès qu'UN message vient d'un
  // connu ; un inconnu qui y répond attend au Portier PENDANT que son
  // message se lit. Le badge le dit, au lieu de laisser croire que le
  // guichet a été contourné. Chargé à l'ouverture du fil, mode
  // organisé seul — le guichet est court, l'appel se compte en ms.
  let attentePortier = $state(new Set());
  $effect(() => {
    void fil.ligne;
    if (!organise || !fil.ligne) {
      attentePortier = new Set();
      return;
    }
    let perime = false;
    appel('screener_addresses')
      .then((adresses) => {
        if (!perime) attentePortier = new Set(adresses);
      })
      .catch(() => {});
    return () => {
      perime = true;
    };
  });
  // La clé du guichet est `sender_norm` (lower(trim()) SQLite, donc
  // ASCII) ; le toLowerCase JS est Unicode — divergence ASSUMÉE sur
  // une majuscule non-ASCII dans l'adresse (la même limite que
  // `adresse_images` côté cœur) : le badge peut manquer, jamais mentir.
  const enAttente = (m) =>
    !!m.sender_address && attentePortier.has(m.sender_address.trim().toLowerCase());

  // Le brouillon du fil ouvert — le plus récent (B-D5).
  const brouillonDuFil = $derived.by(() => {
    if (!fil.ligne || fil.ligne.thread_id == null) return null;
    let retenu = null;
    for (const b of brouillons) {
      if (b.thread_id !== fil.ligne.thread_id) continue;
      if (!retenu || b.updated_epoch > retenu.updated_epoch) retenu = b;
    }
    return retenu;
  });

  // Les puces d'inventaire de la maquette (terrain A45) : n messages
  // TOUJOURS dit — « 1 message » compris —, fichiers SOMMÉS sur le fil
  // (la ligne ne porte que le compte de SON message).
  const nbMessages = $derived(fil.messages.length || fil.ligne?.thread_size || 1);
  // La bascule se DÉRIVE de l'état réel (terrain A47) : tout déplié →
  // « Tout replier » — un fil d'un message s'ouvre donc dessus.
  const tousDeplies = $derived(
    fil.messages.length > 0 && fil.messages.every((m) => fil.deplies[cleMsg(m)]),
  );
  const totalPieces = $derived(
    fil.messages.length
      ? fil.messages.reduce((n, m) => n + (nbPiecesDe(m) || 0), 0)
      : (fil.ligne ? nbPiecesDe(fil.ligne) : 0),
  );

  const propre = (m) => fil.ligne && m.sender_address === fil.ligne.account_email;

  // R5 (PLAN-RETOURS-12, décision D4) : les noms des destinataires
  // viennent de l'ANNUAIRE des correspondants — `to_addrs`/`cc_addrs`
  // ne stockent que des adresses nues (address_literal), et le courrier
  // vu a déjà appris les noms. UNE requête par jeu d'adresses, bornée
  // aux À/Cc du fil (jamais un parcours d'enveloppes — leçon A64) ;
  // adresse inconnue : nue. La clé mémoïse : une repose de
  // `fil.messages` ou un remontage (bascule volet ↔ écran 03) aux mêmes
  // adresses ne repart pas en RPC (revue). L'invalidation vit dans le
  // cleanup de l'effet — le cycle de vie que Svelte fournit.
  let annuaire = $state({});
  $effect(() => {
    const adresses = [...new Set(
      fil.messages.flatMap((m) => [...(m.to_addrs ?? []), ...(m.cc_addrs ?? [])]),
    )];
    const cle = adresses.join('\n');
    // Le cache survit au composant (cacheNoms, lib/fil.svelte.js) : une
    // repose de fil.messages ou un remontage aux mêmes adresses ne
    // repart pas en RPC.
    if (cle === cacheNoms.cle) {
      annuaire = cacheNoms.noms;
      return;
    }
    if (!adresses.length) {
      cacheNoms.cle = '';
      cacheNoms.noms = {};
      annuaire = {};
      return;
    }
    let perime = false;
    appel('address_names', { addresses: adresses }).then(
      (noms) => {
        cacheNoms.cle = cle;
        cacheNoms.noms = noms;
        if (!perime) annuaire = noms;
      },
      // L'échec se DIT (revue) : le repli visible est l'adresse nue,
      // sans ce signal une régression de la commande serait muette.
      (err) => console.error('address_names :', err),
    );
    return () => { perime = true; };
  });
  // LA forme « Nom <adresse> » — une seule règle pour les trois lignes
  // de l'entête : nom absent, vide ou égal à l'adresse → adresse nue.
  const etiquette = (nom, adresse) =>
    (nom && nom !== adresse ? `${nom} <${adresse}>` : adresse);
  // Le nom vient de l'annuaire (clé en minuscules, sa forme).
  const nomAdr = (adresse) => etiquette(annuaire[adresse.trim().toLowerCase()], adresse);
  // Le nom est déjà PORTÉ (l'expéditeur d'une copie du fil).
  const nomAdrPorte = (nom, adresse) => (adresse ? etiquette(nom, adresse) : (nom ?? ''));
  // R4 (PLAN-RETOURS-MAIL) : les destinataires stockés (`to_addrs`,
  // tirés de la même ENVELOPE à la synchro) disent à qui le message est
  // parti. Repli des messages d'avant leur stockage : l'heuristique du
  // prototype — message de soi → premier autre correspondant du fil,
  // message d'autrui → notre propre copie (Envoyés), sinon l'adresse du
  // compte.
  function destinataires(m) {
    if ((m.to_addrs?.length ?? 0) > 0) {
      return m.to_addrs.map(nomAdr).join(', ');
    }
    const vise = propre(m)
      ? fil.messages.find((x) => x.sender_address && x.sender_address !== m.sender_address)
      : fil.messages.find((x) => propre(x));
    if (vise) return nomAdrPorte(vise.sender, vise.sender_address);
    // Dernier repli : l'adresse du compte, nue — le fait honnête (le
    // cœur ne connaît pas notre nom, et l'annuaire n'est requêté que
    // sur les À/Cc du fil).
    return fil.ligne?.account_email ?? '';
  }

  // E5bis : `corpsAuto` vit dans lib/corps.js — le Kiosque en cartes
  // mesure les mêmes corps, une seule porte (A47/S1).

  // La réponse à une invitation (D5-D6) : sujet et corps dans la langue
  // de l'UI, l'email iTIP journalisé côté cœur (reply_invitation),
  // puis une vidange lancée — hors ligne, il part au prochain lancement
  // (la sémantique dite de PLAN-RETOURS-6).
  let reponsesEnVol = $state({});
  async function repondreInvitation(m, reponse) {
    const k = cleMsg(m);
    if (reponsesEnVol[k]) return;
    reponsesEnVol[k] = true;
    // OPTIMISTE (terrain R3'a) : le bouton se marque à l'instant du
    // clic — le journal suit ; un échec rend l'état d'avant et le dit.
    const avant = fil.invitations[k].statut;
    fil.invitations[k].statut = reponse;
    try {
      const sujet = t(`inv.sujet_${reponse}`, { titre: fil.invitations[k].titre });
      const vue = await appel('reply_invitation', {
        accountId: m.account_id,
        mailbox: m.mailbox,
        uid: m.uid,
        reponse,
        sujet,
        corps: sujet,
      });
      if (vue) fil.invitations[k] = vue;
      appel('flush_outbox').catch(() => {});
    } catch (err) {
      fil.invitations[k].statut = avant;
      onflash(t('erreur.invitation', { err }));
    } finally {
      reponsesEnVol[k] = false;
    }
  }

  // En-vol transitoire : local au composant, rien à partager entre
  // cadres (revue v3 — le store ne porte que l'état du fil).
  let enregistrements = $state({});
  async function enregistrer(m, piece) {
    const k = `${cleMsg(m)}#${piece.index}`;
    if (enregistrements[k]) return;
    enregistrements[k] = true;
    try {
      // R1 (PLAN-RETOURS-4, D2) : le chemin proposé (Téléchargements +
      // nom assaini par le cœur), puis le dialogue « Enregistrer sous »
      // — l'utilisateur choisit dossier ET nom. Annuler = rien, ni
      // toast ni erreur ; le rapatriement des octets n'a lieu qu'après
      // le choix (jamais de fetch inutile si l'on renonce).
      const defaut = await appel('suggested_save_path', { name: piece.name });
      const dest = await choisirDestination(defaut);
      if (!dest) return;
      const chemin = await appel('save_attachment', {
        accountId: m.account_id,
        mailbox: m.mailbox,
        uid: m.uid,
        index: piece.index,
        dest,
      });
      onflash(t('toast.pieceEnregistree', { chemin }));
    } catch (err) {
      onflash(t('erreur.enregistrement', { err }));
    } finally {
      enregistrements[k] = false;
    }
  }
</script>

{#if fil.ligne}
  <!-- Les DEUX cadres sont À PLAT (terrain A46, étendu à l'écran 03
       par PLAN-RETOURS-7 R3) : pas d'élévation englobante, pas de
       filets — seules les cartes de message s'élèvent, et tout défile
       en flot SAUF la barre du fil, collante en tête (RETOURS-14 R1).
       « L'écran 03 garde sa carte pleine » (A46) est renversé : le
       cadre plein est une colonne centrée à plat (Conversation.svelte). -->
  <div class="objet-fil">
    <div class="tete">
      <h3 class="titre display" data-testid="fil-sujet">{fil.ligne.subject}</h3>
      <!-- Le rang de la maquette (terrain A45) : puces d'inventaire à
           gauche, boutons NUS à droite — « Tout déplier » au bord. -->
      <div class="puces" data-testid="fil-puces">
        <span class="puce"><Icone nom="forum" />{t('puce.messages', { n: nbMessages })}</span>
        {#if totalPieces > 0}
          <span class="puce"><Icone nom="attach_file" />{t('puce.fichiers', { n: totalPieces })}</span>
        {/if}
        <span class="essor"></span>
        {#if onagrandir}
          <!-- V-D2 : un message SEUL n'a pas de conversation à ouvrir
               — le bouton reste, inerte et dit tel. « Ouvrir » porte
               son propre glyphe (A46) : une icône, un sens (A3). -->
          <button type="button" class="nu" data-testid="voir-conversation"
                  class:inerte={fil.ligne.thread_id == null}
                  aria-disabled={fil.ligne.thread_id == null}
                  tabindex={fil.ligne.thread_id != null ? 0 : -1}
                  onclick={() => fil.ligne.thread_id != null && onagrandir(fil.ligne)}>
            <Icone nom="open_in_full" />{t('lecture.ouvrir')}</button>
        {/if}
        <!-- La bascule (A46, dérivée depuis A47) : « Tout replier »
             quand TOUT est déplié — fil d'un message compris —, sinon
             « Tout déplier » ; les dépliages manuels la font suivre. -->
        {#if tousDeplies}
          <button type="button" class="nu" data-testid="tout-replier" onclick={toutReplier}>
            <Icone nom="unfold_less" />{t('conv.replier')}</button>
        {:else}
          <button type="button" class="nu" data-testid="tout-deplier" onclick={toutDeplier}>
            <Icone nom="unfold_more" />{t('conv.deplier')}</button>
        {/if}
      </div>
    </div>

    <!-- RETOURS-14 R1 (D1) : la barre du fil vit EN TÊTE, collante au
         défilement — elle reste visible au fond d'un long fil, dans
         les DEUX cadres (le scroll appartient au cadre, le sticky
         s'ancre au scrollport du volet comme de la scène). Gestes de
         TRI seuls (D5) : Répondre/Répondre à tous/Transférer restent
         par message (D4). Signaler comme spam s'y range (D2), ou « Ce
         n'est pas un spam » en vue Indésirables. -->
    <!-- LA barre du fil (BarreFil.svelte) — au volet seulement : à
         l'écran 03 ses boutons vivent dans la barre d'entête de la
         scène (Conversation, terrain 2026-09-02). -->
    {#if fil.cadre !== 'plein'}
      <BarreFil {estIndesirable} {epinglable} {organise}
                {onarchiver} {onspam} {onnonspam} {onepingler} {ondeplacer} {oncote} />
    {/if}

    <div class="fil">
      {#each fil.messages as m (cleMsg(m))}
        {@const k = cleMsg(m)}
        {@const boite = boiteDe(m)}
        {#if fil.deplies[k]}
          <article class="deplie" data-testid="message-deplie">
            <!-- L'en-tête en deux lignes (PLAN-RETOURS-12 R5) :
                 « Nom <adresse> sur Boîte » puis « À : Nom <adresse>, … »
                 (et « Cc : … » si des Cc existent, D6) — le bloc
                 De/À/Objet reste mort, la tête dit tout (A45). -->
            <div class="tete-message" role="button" tabindex="0"
                 aria-expanded="true"
                 onclick={() => basculerMessage(m)} onkeydown={activation(() => basculerMessage(m))}>
              <span class="avatar" aria-hidden="true">{initiales(m.sender)}</span>
              <span class="qui">
                <!-- A80/D5 : la boîte derrière le nom — le même bloc
                     que la ligne de liste (systeme.css). -->
                <span class="rang-nom">
                  <span class="auteur">{m.sender}</span>
                  {#if m.sender_address && m.sender_address !== m.sender}
                    <span class="adr adr-exp">{`<${m.sender_address}>`}</span>
                  {/if}
                  {#if enAttente(m)}
                    <span class="attente-portier" data-testid="attente-portier">{t('fil.attentePortier')}</span>
                  {/if}
                  {#if boite}
                    <span class="boite" title={boite.titre}>
                      <span class="mot">{t('liste.sur')}</span>
                      {#if boite.repere}
                        <span class="repere-nu" data-teinte={boite.repere.teinte}
                              aria-hidden="true"><Icone nom={boite.repere.icone} taille={14} /></span>
                      {/if}
                      <span class="lib">{boite.libelle}</span>
                    </span>
                  {/if}
                </span>
                <span class="adr" data-testid="ligne-a">{t('conv.ligneA', { liste: destinataires(m) })}</span>
                {#if (m.cc_addrs?.length ?? 0) > 0}
                  <span class="adr" data-testid="ligne-cc">{t('conv.ligneCc', { liste: m.cc_addrs.map(nomAdr).join(', ') })}</span>
                {/if}
              </span>
              <span class="quand">{quandLong(m.epoch)}</span>
            </div>
            <div class="contenu">
              <!-- La carte d'invitation (PLAN-INVITATIONS, A76) : EN
                   TÊTE du contenu — c'est l'objet du message, avant les
                   fichiers. Tuile de date en tuile/tuileInk (le dessin
                   de la boîte en cours), trois boutons NEUTRES (D4 —
                   A14 intact, la carte ne hiérarchise pas la réponse),
                   la réponse courante dite par aria-pressed. Une heure
                   flottante s'affiche telle quelle, jamais convertie
                   (garde D1). -->
              {#if fil.invitations[k]}
                {@const inv = fil.invitations[k]}
                {@const tuile = tuileInvitation(inv)}
                {@const quandInv = quandInvitation(inv)}
                {@const lieuOrg = lieuOrganisateur(inv)}
                {@const repondantInv = ligneRepondant(inv)}
                <div class="invitation" data-testid="invitation">
                  <div class="inv-tete">
                    <span class="inv-kicker" class:annulee={inv.annulee}>{kickerInvitation(inv)}</span>
                    {#if inv.methode === 'request'}
                      <span class="inv-statut" data-testid="invitation-statut">{statutInvitation(inv)}</span>
                    {/if}
                  </div>
                  <div class="inv-corps">
                    {#if tuile}
                      <span class="inv-tuile" class:eteinte={inv.annulee} aria-hidden="true">
                        <span class="inv-mois">{tuile.mois}</span>
                        <span class="inv-jour">{tuile.jour}</span>
                      </span>
                    {/if}
                    <div class="inv-details">
                      <span class="inv-titre" class:barre={inv.annulee}
                            data-testid="invitation-titre">{inv.titre}</span>
                      {#if quandInv}
                        <span class="inv-quand">{quandInv}</span>
                      {/if}
                      {#if lieuOrg}
                        <span class="inv-lieu">{lieuOrg}</span>
                      {/if}
                      {#if inv.annulee}
                        <span class="inv-annulee">{t('inv.annuleeTexte')}</span>
                      {:else if repondantInv}
                        <span class="inv-repondant" data-testid="invitation-repondant">{repondantInv}</span>
                      {/if}
                    </div>
                  </div>
                  {#if inv.peut_repondre}
                    <!-- R7/R9 (terrain 2026-08-23) : l'icône dit la
                         réponse, la couleur son sens (accent / neutre /
                         alerte) — le texte double toujours (A8). -->
                    <div class="inv-actions" data-testid="invitation-actions">
                      <button type="button" class="ton-accepte" data-testid="inv-accepter"
                              aria-pressed={inv.statut === 'accepte'}
                              disabled={reponsesEnVol[k]}
                              onclick={() => repondreInvitation(m, 'accepte')}>
                        <Icone nom="check_circle" />{t('action.accepter')}</button>
                      <button type="button" class="ton-provisoire" data-testid="inv-provisoire"
                              aria-pressed={inv.statut === 'provisoire'}
                              disabled={reponsesEnVol[k]}
                              onclick={() => repondreInvitation(m, 'provisoire')}>
                        <Icone nom="question_mark" />{t('action.provisoire')}</button>
                      <button type="button" class="ton-refuse" data-testid="inv-refuser"
                              aria-pressed={inv.statut === 'refuse'}
                              disabled={reponsesEnVol[k]}
                              onclick={() => repondreInvitation(m, 'refuse')}>
                        <Icone nom="cancel" />{t('action.refuser')}</button>
                    </div>
                  {/if}
                </div>
              {/if}
              <!-- R2 (PLAN-RETOURS-7) : les fichiers joints AVANT le
                   corps — sous la tête du message, où l'œil les attend
                   sans dérouler le mail ; la garde d'images reste collée
                   au corps qu'elle concerne. -->
              <!-- Un écho de GESTE n'a pas de métadonnées par pièce
                   (elles meurent avec la source) : la section ne se
                   montre que si des puces existent — jamais un titre
                   sans rien dessous (PLAN-RETOURS-5). -->
              {#if nbPiecesDe(m) > 0 && (!estEcho(m) || (fil.pieces[k] ?? []).length > 0)}
                <div class="fichiers" data-testid="lecture-fichiers">
                  <p class="titre-fichiers">{t('conv.fichiersJoints')}</p>
                  <!-- R2 (PLAN-RETOURS-4, D4) : nom ET poids dans la MÊME
                       puce cliquable — exception assumée à « 1 puce = 1
                       information », icône unique (même objet manipulable
                       que la puce du composeur, pas deux lectures). -->
                  <!-- Les puces d'un écho sont INERTES (PLAN-RETOURS-5,
                       D2) : les octets ont quitté le journal à l'envoi —
                       nom et poids se montrent, rien ne s'enregistre
                       pendant la fenêtre de réconciliation — et ne
                       portent donc AUCUN voile (aucune promesse). -->
                  <div class="puces">
                    {#each fil.pieces[k] ?? [] as piece (piece.index)}
                      <button type="button" class="puce bouton" data-testid="piece-jointe"
                              disabled={estEcho(m) || enregistrements[`${k}#${piece.index}`]}
                              onclick={() => !estEcho(m) && enregistrer(m, piece)}
                              title={estEcho(m) ? undefined : t('lecture.enregistrer')}>
                        <Icone nom="description" />
                        <span class="nom">{piece.name}</span><span class="taille">{piece.size}</span>
                        <!-- R1 (PLAN-RETOURS-7, D1) : au survol comme au
                             focus clavier, un voile couvre la puce et DIT
                             l'action — « Enregistrer » (le vocabulaire du
                             produit : le clic ouvre « Enregistrer sous »).
                             Même géométrie, la rangée ne reflue pas. -->
                        {#if !estEcho(m)}
                          <span class="voile" aria-hidden="true">
                            <Icone nom="download" />{t('lecture.voileEnregistrer')}</span>
                        {/if}
                      </button>
                    {/each}
                  </div>
                </div>
              {/if}
              {#if (fil.imagesBloquees[k] ?? 0) > 0}
                <div class="garde-images" data-testid="garde-images">
                  <Icone nom="visibility_off" />
                  <span class="garde-texte">{t('lecture.imagesBloquees', { n: fil.imagesBloquees[k] })}</span>
                  <button type="button" data-testid="afficher-images"
                          onclick={() => afficherImages(m)}>
                    {t('lecture.afficherImages')}</button>
                  <!-- D3 (RETOURS-11) : la règle d'expéditeur — jamais
                       sur un écho (soi-même, pas d'expéditeur tiers). -->
                  {#if !estEcho(m)}
                    <button type="button" data-testid="toujours-afficher-images"
                            onclick={() => toujoursAfficherImages(m)}>
                      {t('lecture.toujoursAfficherImages')}</button>
                  {/if}
                </div>
              {/if}
              {#if fil.erreurs[k]}
                <!-- PLAN-AUDIT-V2 E10 : le coeur n'a pas servi ce corps —
                     la grammaire de la garde d'images, avec le geste qui
                     rejoue (avant : un cadre vide, definitif). -->
                <div class="garde-images" data-testid="corps-echec">
                  <Icone nom="error" />
                  <span class="garde-texte">{t('lecture.corpsEchec')}</span>
                  <button type="button" data-testid="corps-reessayer"
                          onclick={() => reessayer(m)}>
                    {t('action.reessayer')}</button>
                </div>
              {/if}
              <iframe class="corps" sandbox="allow-same-origin" srcdoc={fil.corps[k] ?? ''}
                      title={t('lecture.corps')} use:corpsAuto
                      onload={(ev) => brancherLiens(ev.currentTarget)}></iframe>
            </div>
            <!-- R4 (PLAN-RETOURS-3, D4) : les gestes de réponse EN BAS de
                 CHAQUE message, visant CE message — on répond quand on a
                 fini de lire (convention Gmail/Outlook). « Répondre à
                 tous » entre Répondre et Transférer (A14). -->
            <!-- R4 (constat terrain 2026-08-18) : les TROIS gestes sur
                 CHAQUE message, le nôtre compris — on répond parfois sur
                 son propre message. Le cœur adresse alors la réponse aux
                 destinataires d'origine (reply_context/reply_all), jamais
                 à soi-même. -->
            <div class="actions-message" data-testid="actions-message">
              <button type="button" class="principal" data-testid="repondre"
                      onclick={() => onrepondre(m)}>
                <Icone nom="reply" />{t('action.repondre')}</button>
              <button type="button" data-testid="repondre-tous"
                      onclick={() => onrepondretous(m)}>
                <Icone nom="reply_all" />{t('action.repondreTous')}</button>
              <button type="button" data-testid="transferer"
                      onclick={() => ontransferer(m)}>
                <Icone nom="reply" miroir />{t('action.transferer')}</button>
              <!-- Terrain R8' (2026-08-23) : « Supprimer » vit PAR
                   message — on supprime CE message, pas la
                   conversation ; le fil reste ouvert s'il lui reste
                   des messages (l'App décide). Sur un écho, le geste
                   dit l'attente de réconciliation, comme avant. -->
              <button type="button" data-testid="supprimer"
                      onclick={() => onsupprimer(m)}>
                <Icone nom="delete" />{t('action.supprimer')}</button>
            </div>
          </article>
        {:else}
          <div class="replie" data-testid="message-replie"
               role="button" tabindex="0" aria-expanded="false"
               onclick={() => basculerMessage(m)} onkeydown={activation(() => basculerMessage(m))}>
            <span class="avatar petit" aria-hidden="true">{initiales(m.sender)}</span>
            <span class="auteur">{m.sender}</span>
            {#if enAttente(m)}
              <span class="attente-portier" data-testid="attente-portier">{t('fil.attentePortier')}</span>
            {/if}
            <!-- A80/D5 : la boîte derrière le nom, ici aussi. -->
            {#if boite}
              <span class="boite" title={boite.titre}>
                <span class="mot">{t('liste.sur')}</span>
                {#if boite.repere}
                  <span class="repere-nu" data-teinte={boite.repere.teinte}
                        aria-hidden="true"><Icone nom={boite.repere.icone} taille={14} /></span>
                {/if}
                <span class="lib">{boite.libelle}</span>
              </span>
            {/if}
            <span class="apercu">{m.preview ?? ''}</span>
            <span class="quand">{quandLong(m.epoch)}</span>
          </div>
        {/if}
      {/each}
      {#if brouillonDuFil}
        <div class="replie brouillon" data-testid="conv-brouillon"
             role="button" tabindex="0"
             onclick={() => onreprendre(brouillonDuFil)}
             onkeydown={activation(() => onreprendre(brouillonDuFil))}>
          <span class="mention"><Icone nom="edit_note" />{t('conv.brouillon')}</span>
          <span class="apercu">{brouillonDuFil.body}</span>
          <span class="quand">{quand(Math.floor(brouillonDuFil.updated_epoch / 1000))}</span>
          <span class="reprendre">{t('action.reprendre')}</span>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  /* La forme À PLAT (A46, les deux cadres depuis PLAN-RETOURS-7 R3) —
     la géométrie du prototype (.voletLecture / .lecture) : le fil
     défile en un seul flot dans son cadre (le volet ou la scène de
     l'écran 03), les filets et l'élévation appartiennent aux seules
     cartes ; seule la largeur disponible change entre cadres. */
  .objet-fil { display:flex; flex-direction:column; flex:none; min-height:100%; padding-top:var(--fil-haut, 0); }
  .tete { display:flex; flex-direction:column; flex:none; }
  /* V6 : le titre passe au registre d'affichage (graisse 340,
     -.03em — classe globale .display) ; la taille reste 24 px. */
  .titre {
    margin:2px 0 4px; font-size:24px; line-height:1.2;
    color:var(--ink);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .puces { display:flex; align-items:center; gap:10px; flex-wrap:wrap; margin:0 0 4px; }
  .puce {
    height:32px; padding:0 12px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink2); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle); white-space:nowrap;
  }
  .puce.bouton { cursor:pointer; }
  .puce.bouton:hover { background:var(--sel); }
  /* Les boutons NUS de la maquette (A45) : bordure et fond effacés,
     gabarit mini 26 px — « Tout déplier », « Voir la conversation ». */
  .essor { flex:1; }
  .nu {
    height:26px; padding:0 9px; display:inline-flex; align-items:center;
    gap:6px; font-size:12px; color:var(--ink2); background:none;
    border:1px solid transparent; border-radius:var(--r-controle); cursor:pointer;
    white-space:nowrap;
  }
  .nu:hover { background:var(--sel); }
  .nu.inerte { cursor:default; opacity:.55; }
  .nu.inerte:hover { background:none; }
  /* L'avatar aux initiales des cartes (A45) — le dessin de la liste
     (E2) : 28 px déplié, 26 px replié. */
  /* V4 — la tuile carrée d'initiales : sol --tuile, encre --tuileInk,
     filet 1 px (mesuré : sans lui la tuile n'existe pas). */
  .avatar {
    width:28px; height:28px; border-radius:var(--r-tuile);
    background:var(--tuile);
    border:1px solid var(--border); display:grid; place-items:center;
    font-size:11px; font-weight:600; color:var(--tuileInk); flex:none;
  }
  .avatar.petit { width:26px; height:26px; }
  .fil { flex:none; overflow-y:visible; padding:0; }
  .replie {
    display:flex; align-items:center; gap:10px; padding:12px 20px;
    margin-top:12px; background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-surface); cursor:pointer;
    font-size:13px;
  }
  .replie:hover { background:var(--hover); }
  .replie .auteur { font-weight:600; color:var(--ink); flex:none; }
  .replie .apercu {
    flex:1; min-width:0; color:var(--muted);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .quand { margin-left:auto; color:var(--muted); font-size:12px; flex:none; white-space:nowrap; }
  .replie.brouillon { border:1.5px dashed var(--accent); background:none; }
  .replie.brouillon .mention {
    color:var(--alert); font-weight:600; display:inline-flex;
    align-items:center; gap:6px; flex:none;
  }
  .replie.brouillon .reprendre { color:var(--accent); font-weight:600; flex:none; }
  .deplie {
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-surface); box-shadow:var(--shadow); margin-top:12px;
    display:flex; flex-direction:column;
  }
  /* L'en-tête A92 : avatar · (nom <adresse> [sur boîte] / À : … / Cc : …) · quand. */
  .tete-message {
    display:flex; align-items:center; gap:10px; padding:12px 20px;
    border-bottom:1px solid var(--border); cursor:pointer;
  }
  /* `flex:1 1 auto` : sans lui, .qui se dimensionnait au contenu et le
     plafond du tiers de .boite se résolvait contre ce groupe étroit —
     la règle écrite au Système (« jamais plus du tiers de la LIGNE »)
     ne décrivait pas ce que le fil rendait (revue). */
  .tete-message .qui { min-width:0; flex:1 1 auto; display:flex; flex-direction:column; }
  /* A80/D5 : nom + bloc de boîte sur la même ligne — le bloc
     (systeme.css) garde son plafond du tiers et cède le premier. */
  .tete-message .rang-nom {
    display:flex; align-items:baseline; gap:6px; min-width:0;
  }
  .tete-message .auteur {
    flex:0 1 auto; min-width:0;
    font-size:15px; font-weight:600; color:var(--ink);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .tete-message .adr {
    font-size:12px; color:var(--muted);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  /* R5 : l'adresse de l'expéditeur, en ligne 1 derrière le nom — elle
     PORTE .adr (même encre que les lignes À/Cc, structurellement) et
     n'ajoute que sa règle de cession : TROIS fois plus vite que le nom
     (le patron d'A80 : l'identité d'abord, le détail cède). */
  .tete-message .adr-exp { flex:0 3 auto; min-width:0; }
  .contenu { padding:14px 20px 18px; display:flex; flex-direction:column; gap:12px; }
  /* La carte d'invitation (A76) : une carte DANS la carte de message —
     rayon surface 10 px, sans élévation (elle appartient au flot du
     contenu, pas au fil). La tuile de date reprend la paire
     --tuile/--tuileInk de la boîte en cours ; l'annulation passe la
     tuile en éteint et le titre en barré. */
  .invitation { border:1px solid var(--border); border-radius:var(--r-surface); background:var(--surface); }
  .inv-tete { display:flex; align-items:center; gap:10px; padding:12px 14px 0; }
  .inv-kicker {
    font-size:12px; font-weight:600; letter-spacing:.1em;
    text-transform:uppercase; color:var(--muted); flex:1;
  }
  .inv-kicker.annulee { color:var(--alert); }
  .inv-statut { font-size:12px; color:var(--ink2); white-space:nowrap; }
  .inv-corps { display:flex; gap:14px; padding:12px 14px 14px; align-items:flex-start; }
  .inv-tuile {
    width:52px; height:52px; border-radius:var(--r-controle); background:var(--tuile);
    color:var(--tuileInk); display:flex; flex-direction:column;
    align-items:center; justify-content:center; gap:1px; flex:none;
  }
  .inv-tuile.eteinte { background:var(--bg); color:var(--muted); }
  .inv-mois {
    font-size:10px; font-weight:600; letter-spacing:.08em;
    text-transform:uppercase;
  }
  .inv-jour { font-size:20px; font-weight:600; line-height:1; }
  .inv-details { display:flex; flex-direction:column; gap:4px; min-width:0; }
  .inv-titre { font-size:15px; font-weight:600; color:var(--ink); }
  .inv-titre.barre { color:var(--ink2); text-decoration:line-through; }
  .inv-quand { font-size:13px; color:var(--ink2); }
  .inv-lieu { font-size:13px; color:var(--muted); }
  .inv-annulee { font-size:13px; color:var(--alert); }
  .inv-repondant { font-size:13px; font-weight:600; color:var(--ink2); }
  /* Trois boutons NEUTRES (D4) au gabarit des actions de message
     (30 px) ; la réponse courante se dit par aria-pressed — fond --sel
     et liseré d'accent, la sélection d'A75. */
  .inv-actions {
    display:flex; gap:10px; padding:12px 14px;
    border-top:1px solid var(--border); flex-wrap:wrap;
  }
  .inv-actions button:disabled { cursor:default; opacity:.55; }
  /* R9 : la couleur dit le sens — portée par l'icône, le texte double
     (A8). Paires gatées : accent/surface et alert/surface à 3:1,
     muted/surface à 4,5:1, et leurs pendants sur --sel. */
  .inv-actions .ton-accepte :global(.ic) { color:var(--accent); }
  .inv-actions .ton-provisoire :global(.ic) { color:var(--muted); }
  .inv-actions .ton-refuse :global(.ic) { color:var(--alert); }
  .inv-actions button[aria-pressed='true'] {
    font-weight:600; background:var(--sel); border-color:var(--accent);
  }
  .garde-images {
    padding:10px 14px; display:flex; align-items:center; gap:10px;
    font-size:13px; color:var(--ink2); background:var(--bg);
    border:1px solid var(--border); border-radius:var(--r-controle);
    /* Deux boutons depuis RETOURS-11 (D3) : en fenêtre étroite ils
       passent à la ligne plutôt que d'écraser le texte. */
    flex-wrap:wrap;
  }
  .garde-images :global(.ic) { color:var(--muted); }
  .garde-texte { flex:1; }
  .garde-images button {
    height:26px; padding:0 10px; font-size:12px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); cursor:pointer;
  }
  .garde-images button:hover { background:var(--sel); }
  .corps {
    /* Déborde de 12 px : la gouttière interne du document assaini
       (mail-render) ramène le texte au fil du padding de la carte. Le
       fond au jeton — le document bake la même valeur (revue A42).
       La HAUTEUR n'est pas ici : elle suit le contenu, posée par
       corpsAuto (A47) — jamais de gabarit fixe. */
    border:none; background:var(--surface); display:block;
    margin-left:-12px; width:calc(100% + 24px); height:0;
  }
  .titre-fichiers {
    margin:0 0 8px; font-size:12px; font-weight:600; letter-spacing:.1em;
    text-transform:uppercase; color:var(--muted);
  }
  .fichiers .puces { gap:8px; }
  .fichiers .puce { height:28px; }
  /* R2 : nom + poids dans la puce (dessin de la puce du composeur) —
     le nom à l'encre pleine, le poids atténué, l'espacement au gap. */
  .fichiers .puce .nom { color:var(--ink); }
  .fichiers .puce .taille { font-size:12px; color:var(--muted); }
  /* RETOURS-14 R4 (D5) : le badge « En attente au Portier » — une
     étiquette nue à l'encre atténuée, filet de bordure, jamais une
     alerte : le courrier est légitime, son verdict est juste dû. */
  .attente-portier {
    flex:none; padding:1px 6px; font-size:11px; color:var(--ink2);
    border:1px solid var(--border); border-radius:var(--r-controle);
    white-space:nowrap;
  }
  /* E1 : le menu de « Déplacer vers… » — une carte SOUS le bouton
     depuis que la barre vit en tête (RETOURS-14 R1 ; l'idiome du
     nuancier, A62), boutons au gabarit de la barre, texte à gauche. */
  /* R4/D4 : la barre de réponse d'UN message — en bas de la carte.
     Terrain 2026-09-02 (CE, passe 3 du STOP 2 de la vague 2) : elle
     FLOTTE en bas du message — l'objet flottant du produit (A108 :
     surface, bordure, rayon des contrôles, ombre --shadow), collante
     au bas du scrollport tant que le message défile (12 px de marge
     avec le pied), et en place dans la carte quand sa fin arrive, à
     12/20/16 px des bords de l'élévation. `align-self:flex-start` :
     elle se resserre sur ses boutons, elle ne barre pas la carte. */
  .actions-message {
    position:sticky; bottom:12px; align-self:flex-start;
    margin:12px 20px 16px; padding:8px 10px;
    display:flex; gap:10px; flex-wrap:wrap;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); box-shadow:var(--shadow);
  }
  /* UN gabarit pour les boutons de message ET ceux de la carte
     d'invitation (A76 dit « au gabarit des actions de message ») : le
     tenir par copie divergerait au premier retunage (revue). */
  .actions-message button, .inv-actions button {
    height:30px; padding:0 14px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle); cursor:pointer;
  }
  .actions-message button:hover, .inv-actions button:hover { background:var(--sel); }
  .actions-message .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .actions-message .principal:hover { background:var(--accentH); border-color:var(--accentH); }

  /* R1 (PLAN-RETOURS-7, D1) : le voile d'une puce de pièce — même
     géométrie que la puce (recouvrement absolu, largeur stable, la
     rangée ne reflue pas), fond --sel opaque (la paire encre/sel est
     celle du survol existant), glyphe download + « Enregistrer ».
     Montré au survol ET au focus clavier (A8) ; jamais pendant un
     enregistrement en vol (:disabled) ni sur un écho (non rendu). */
  .puce.bouton { position:relative; }
  .puce .voile {
    position:absolute; inset:0; display:none; align-items:center;
    justify-content:center; gap:6px; font-size:12px; font-weight:600;
    color:var(--ink); background:var(--sel); border-radius:var(--r-controle);
    white-space:nowrap; overflow:hidden;
  }
  .puce.bouton:hover .voile, .puce.bouton:focus-visible .voile { display:inline-flex; }
  .puce.bouton:disabled .voile { display:none; }
</style>
