// La carte d'invitation (PLAN-INVITATIONS) : mise en forme PURE de la
// vue servie par le cœur (elle voyage avec le corps, BodyView) —
// horaires, statuts, lignes. La langue vient du catalogue ; la grammaire
// des dates est celle de quand.js (dateAbsolue, quand.mois en abrégés).
//
// Garde D1, par EXTRÉMITÉ : une heure FLOTTANTE (TZID inconnu du cœur)
// s'affiche telle quelle, suffixée « heure locale de l'organisateur » —
// jamais convertie ; un couple début-résolu/fin-flottante ne se compacte
// jamais en une plage qui mentirait (revue).
import { t } from './texte.svelte.js';
import { dateAbsolue } from './quand.js';

const pad = (n) => String(n).padStart(2, '0');

// Les composantes locales d'un horaire de la vue : epoch résolu → heure
// du poste ; sinon la forme texte du cœur (AAAA-MM-JJ ou
// AAAA-MM-JJTHH:MM), lue telle quelle — `flottante` le dit.
function composantes(inv, quel) {
  const epoch = inv[`${quel}_epoch`];
  if (epoch != null) {
    const d = new Date(epoch * 1000);
    return {
      annee: d.getFullYear(), mois: d.getMonth(), jour: d.getDate(),
      heure: `${pad(d.getHours())}:${pad(d.getMinutes())}`,
      flottante: false,
    };
  }
  const texte = inv[`${quel}_text`];
  if (!texte) return null;
  const [date, heure] = texte.split('T');
  const [annee, mois, jour] = date.split('-').map(Number);
  return { annee, mois: mois - 1, jour, heure: heure ?? null, flottante: heure != null };
}

// La tuile de date : mois abrégé + quantième — le repère visuel de la
// carte, dessiné en --tuile/--tuileInk (la paire de la boîte en cours).
export function tuileInvitation(inv) {
  const debut = composantes(inv, 'start');
  if (!debut) return null;
  return { mois: t('quand.mois')[debut.mois], jour: String(debut.jour) };
}

// « Jeudi 3 sept. » — jour de semaine + la grammaire de quand.js.
function dateLongue(c) {
  const semaine = t('quand.jours')[new Date(c.annee, c.mois, c.jour).getDay()];
  return `${semaine} ${dateAbsolue(c.annee, c.mois, c.jour)}`;
}

const memeJour = (a, b) =>
  b && a.annee === b.annee && a.mois === b.mois && a.jour === b.jour;

export function quandInvitation(inv) {
  const debut = composantes(inv, 'start');
  if (!debut) return '';
  const fin = composantes(inv, 'end');
  let ligne;
  if (inv.all_day || !debut.heure) {
    ligne = dateLongue(debut);
  } else if (fin?.heure && memeJour(debut, fin) && debut.flottante === fin.flottante) {
    // La plage compacte exige des extrémités de MÊME résolution : mêler
    // une heure convertie et une heure flottante fabriquerait
    // « 14:30 – 13:30 » — la conversion mensongère que D1 interdit.
    ligne = `${dateLongue(debut)} · ${debut.heure} – ${fin.heure}`;
  } else if (fin?.heure) {
    ligne = `${dateLongue(debut)}, ${debut.heure} – ${dateLongue(fin)}, ${fin.heure}`;
  } else {
    ligne = `${dateLongue(debut)} · ${debut.heure}`;
  }
  if (debut.flottante || fin?.flottante) ligne += ` (${t('inv.heureLocale')})`;
  if (inv.recurrent) ligne += ` · ${t('inv.seRepete')}`;
  return ligne;
}

export function kickerInvitation(inv) {
  // L'annulation prime (lien croisé R6) : le REQUEST d'une réunion
  // annulée dit « Invitation annulée » comme le CANCEL lui-même.
  if (inv.cancelled) return t('inv.kickerAnnulee');
  if (inv.method === 'reply') return t('inv.kickerReponse');
  return t('inv.kicker');
}

// Les icônes des réponses (R7/R9) : le pendant rond de check_circle
// pour refuser (`cancel` — jamais `close`, qui garde son sens
// « fermer », A3), le point d'interrogation pour provisoire. Le ton
// porte la couleur (accent / neutre / alerte) — le texte double
// toujours l'icône (A8).
export const ICONES_REPONSE = {
  accepted: 'check_circle',
  tentative: 'question_mark',
  declined: 'cancel',
};

// La puce du rang de liste (R11) : l'annulation prime, sinon la
// réponse donnée. `null` = rien à dire (les gestes prennent la place).
export function puceInvitation(badge) {
  if (badge.cancelled) {
    return { texte: t('inv.puce_annulee'), icon: null, ton: 'annulee' };
  }
  if (ICONES_REPONSE[badge.reply]) {
    return {
      texte: t(`inv.puce_${badge.reply}`),
      icon: ICONES_REPONSE[badge.reply],
      ton: badge.reply,
    };
  }
  return null;
}

// Le statut d'une invitation à répondre — D6 : la vue porte déjà la
// DERNIÈRE réponse partie de Wind, sinon le PARTSTAT lu du message.
export function statutInvitation(inv) {
  if (inv.method !== 'request') return '';
  return inv.status && inv.status !== 'no_reply'
    ? t(`inv.vous_${inv.status}`)
    : t('inv.sansReponse');
}

// « Sofia Nardi a accepté » — le REPLY reçu quand nous organisons.
export function ligneRepondant(inv) {
  if (inv.method !== 'reply' || !inv.attendee) return '';
  const status = ['accepted', 'tentative', 'declined'].includes(inv.attendee_status)
    ? inv.attendee_status
    : 'no_reply';
  return t(`inv.repondant_${status}`, { qui: inv.attendee });
}

export function lieuOrganisateur(inv) {
  if (inv.location && inv.organizer) {
    return t('inv.lieuOrganisateur', { location: inv.location, qui: inv.organizer });
  }
  if (inv.location) return inv.location;
  if (inv.organizer) return t('inv.organisePar', { qui: inv.organizer });
  return '';
}
