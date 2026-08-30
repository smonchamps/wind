// Le jeu d'icônes « Elements » (V8 — PLAN-ELEMENTS) : 87 glyphes
// (78 à V8 ; `check` A86, `feedback` RETOURS-11, 5 Mode organisé A96,
// `nettoyage` A103)
// dessinés en SVG, grille 24, trait 2 unités, bouts nets (butt),
// jonctions vives (miter). Ce catalogue est LE jeu livré ; le Système
// (docs/design/systeme.dc.html, section Icônes) porte le relevé — un
// glyphe, un sens, un emploi — et les notes de dessin. La gate
// coherence-systeme.mjs tient les deux égaux, dans les deux sens, et
// vérifie chaque tracé (A18 : une assertion, plus une promesse).
//
//   d        tracés au trait (stroke currentColor)
//   disque   [cx, cy, r] — le disque d'état (--marque), UNIQUE au jeu
//            (mark_email_unread : c'est sa fonction)
//   barre    le seul élément coloré légitime (format_color_text)
//   pleins   [cx, cy, r] disques pleins à l'encre (currentColor)
//   remplis  chemins pleins à l'encre (currentColor)
//   r        RÉSERVÉ au jeu, employé nulle part (A53/A60/A62)
//   repere   glyphe du jeu dédié aux repères de compte (A74)

export const JEU = {
  inbox: { d:['M4 6h16v12H4z','M4 13h5l2 2h2l2-2h5'] },
  all_inbox: { d:['M4 12h16v7H4z','M6 9h12','M8 6h8'] },
  drafts: { d:['M4 19V9l8-4 8 4v10z','M4 9l8 4 8-4'] },
  archive: { d:['M3 5h18v4H3z','M5 9h14v10H5z','M9 13h6'] },
  inventory_2: { d:['M3 4h18v5H3z','M5 9h14v11H5z','M10 6h4'] },
  delete: { d:['M4 7h16','M6 7v12h12V7','M10 4h4','M10 11v5','M14 11v5'] },
  report: { d:['M9 3h6l6 6v6l-6 6H9l-6-6V9z','M12 8v5','M12 16v2'] },
  warning: { d:['M12 4l9 16H3z','M12 10v5'] },
  send: { d:['M3 4l18 8-18 8 3-8z'] },
  mark_email_unread: { d:['M3 8h14v10H3z','M3 8l7 5 7-5'], disque:[19,6,3.4] },
  forum: { d:['M6 5V2h16v10h-6','M2 5h14v9H7l-5 5z'] },
  description: { d:['M6 3h8l4 4v14H6z','M9 12h6','M9 16h4'] },
  bookmark: { d:['M7 3h10v18l-5-5-5 5z'] },
  edit_note: { d:['M4 6h10','M4 11h7','M11 19l1-3 6-6 2 2-6 6z'] },
  edit_square: { d:['M4 6h8','M4 6v14h14v-8','M11 15l1-3 6-6 2 2-6 6z'] },
  signature: { d:['M3 12l4 4','M7 12l-4 4','M10 17V7h5v6h-5','M17 17v-6h5v6','M3 21h4','M10 21h4','M17 21h4'] },
  reply: { d:['M9 6L3 12l6 6','M3 12h11a6 6 0 0 1 6 6v1'] },
  reply_all: { d:['M8 6L2 12l6 6','M14 6l-6 6 6 6','M8 12h8a5 5 0 0 1 5 5v2'] },
  attach_file: { d:['M19 7v9a6 6 0 0 1-12 0V6a4 4 0 0 1 8 0v10a2 2 0 0 1-4 0V8'] },
  keep: { d:['M10 3h4v7l2 4H8l2-4z','M12 14v7'] },
  keep_off: { d:['M10 3h4v7l2 4H8l2-4z','M12 14v7','M4 4l16 16'] },
  open_in_full: { d:['M14 4h6v6','M10 20H4v-6','M20 4l-7 7','M4 20l7-7'] },
  open_in_new: { d:['M14 4h6v6','M20 4l-8 8','M18 13v6H5V6h6'], r:true },
  unfold_more: { d:['M7 10l5-5 5 5','M7 14l5 5 5-5'] },
  unfold_less: { d:['M7 5l5 5 5-5','M7 19l5-5 5 5'] },
  download: { d:['M12 4v10','M8 11l4 4 4-4','M4 19h16'] },
  system_update_alt: { d:['M4 4h16v3H4z','M12 9v8','M8 14l4 4 4-4','M4 21h16'] },
  visibility_off: { d:['M3 12l4-4h10l4 4-4 4H7z','M4 4l16 16'] },
  arrow_back: { d:['M20 12H4','M10 6l-6 6 6 6'] },
  close: { d:['M5 5l14 14','M19 5L5 19'] },
  menu: { d:['M4 7h16','M4 12h16','M4 17h16'] },
  search: { d:['M11 4a7 7 0 1 0 0 14 7 7 0 1 0 0-14','M16 16l4 4'] },
  sync: { d:['M20 12a8 8 0 1 1-2.3-5.7','M20 4v5h-5'] },
  check_circle: { d:['M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16','M8 12l3 3 5-5'] },
  check: { d:['M4 13l5 5L20 7'] },
  cancel: { d:['M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16','M9 9l6 6','M15 9l-6 6'] },
  error: { d:['M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16','M12 7v6','M12 15v2'] },
  info: { d:['M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16','M12 11v6','M12 7v2'] },
  question_mark: { d:['M8 8a4 4 0 1 1 4 4v3','M12 18v2'] },
  priority_high: { d:['M12 4v10','M12 17v3'] },
  hourglass_empty: { d:['M6 3h12l-6 9 6 9H6l6-9z'] },
  schedule_send: { d:['M9 3a6 6 0 1 0 0 12 6 6 0 1 0 0-12','M9 6v3l2 2'], remplis:['M14 15l8 4-8 4 2-4z'] },
  link: { d:['M10 8H8a4 4 0 0 0 0 8h2','M14 8h2a4 4 0 0 1 0 8h-2','M9 12h6'], r:true },
  link_off: { d:['M10 8H8a4 4 0 0 0 0 8h2','M14 8h2a4 4 0 0 1 0 8h-2','M4 4l16 16'] },
  notifications: { d:['M7 17v-6a5 5 0 0 1 10 0v6','M4 17h16','M10 20h4'] },
  person: { d:['M12 5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 1 0 0-7','M5 20v-1a7 7 0 0 1 14 0v1'] },
  person_add: { d:['M9 5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 1 0 0-7','M2 20v-1a7 7 0 0 1 14 0v1','M19 4v6','M16 7h6'] },
  group_add: { d:['M8 5a3 3 0 1 0 0 6 3 3 0 1 0 0-6','M2 19v-1a6 6 0 0 1 12 0v1','M19 4v6','M16 7h6'] },
  // Le retour beta (PLAN-RETOURS-11 R3) : bulle a queue + deux lignes
  // de texte. `forum` garde « conversation » (la puce de fil).
  feedback: { d:['M4 4h16v12H10l-4 4v-4H4z','M8 9h8','M8 12h5'] },
  settings: { d:['M4 8h16','M14 5v6','M4 16h16','M9 13v6'] },
  display_settings: { d:['M3 5h18v12H3z','M6 11h12','M10 9v4','M9 20h6'] },
  keyboard: { d:['M3 7h18v10H3z','M7 14h10','M7 10h2','M11 10h2','M15 10h2'] },
  format_bold: { d:['M8 4v16','M8 4h6l3 4-3 4H8','M8 12h7l3 4-3 4H8'] },
  format_italic: { d:['M10 4h8','M6 20h8','M14 4l-4 16'] },
  format_underlined: { d:['M7 4v8l5 5 5-5V4','M5 20h14'] },
  strikethrough_s: { d:['M16 7h-6l-2 3 8 4-2 3H8','M4 12h16'] },
  format_color_text: { d:['M6 15l6-11 6 11','M9 11h6'], barre:'M4 19h16' },
  format_align_left: { d:['M4 6h16','M4 10h10','M4 14h16','M4 18h10'] },
  format_align_center: { d:['M4 6h16','M7 10h10','M4 14h16','M7 18h10'] },
  format_align_right: { d:['M4 6h16','M10 10h10','M4 14h16','M10 18h10'] },
  format_list_bulleted: { d:['M4 6h2','M4 12h2','M4 18h2','M10 6h10','M10 12h10','M10 18h10'] },
  format_list_numbered: { d:['M11 5h10','M11 12h10','M11 19h10','M5 2v6','M3 8h4','M2 9h5v3H2v3h5','M2 16h5v3H3','M7 19v3H2'] },
  format_indent_increase: { d:['M4 6h16','M11 10h9','M11 14h9','M4 18h16','M5 10l3 2-3 2'] },
  format_indent_decrease: { d:['M4 6h16','M11 10h9','M11 14h9','M4 18h16','M8 10l-3 2 3 2'] },
  format_clear: { d:['M3 5h12v5H3z','M15 7h4v6h-7v8','M2 2l20 20'] },
  format_quote: { d:['M5 14V9h5v5l-3 4H5z','M14 14V9h5v5l-3 4h-2z'], r:true },
  storage: { d:['M3 5h18v4H3z','M3 10h18v4H3z','M3 15h18v4H3z'], r:true },
  account_balance: { d:['M3 9l9-5 9 5','M5 9v9','M10 9v9','M14 9v9','M19 9v9','M3 20h18'], repere:true },
  eco: { d:['M5 19L19 5','M5 19c0-9 6-14 15-14 0 9-6 14-15 14z'], repere:true },
  favorite: { d:['M12 20S4 14 4 9a4 4 0 0 1 8-1 4 4 0 0 1 8 1c0 5-8 11-8 11z'], repere:true },
  flight: { d:['M11 3h2l1 8 7 3v2l-7-1-1 5 3 2v1l-4-1-4 1v-1l3-2-1-5-7 1v-2l7-3z'], repere:true },
  home: { d:['M3 11l9-7 9 7','M6 10v10h12V10'], repere:true },
  music_note: { d:['M11 18V6','M11 6l5 3'], pleins:[[8,18,3]], repere:true },
  pets: { d:['M12 21a4 4 0 0 1-4-4c0-3 8-3 8 0a4 4 0 0 1-4 4z'], pleins:[[4,10,2],[9,6,2],[15,6,2],[20,10,2]], repere:true },
  school: { d:['M2 9l10-4 10 4-10 4z','M6 12v5c0 2 12 2 12 0v-5'], repere:true },
  shopping_bag: { d:['M5 7h14v13H5z','M9 7V5a3 3 0 0 1 6 0v2'], repere:true },
  sports_esports: { d:['M4 16l2-7h12l2 7a2 2 0 0 1-3 1l-2-2H9l-2 2a2 2 0 0 1-3-1z'], remplis:['M7 11h2v2H7z','M15 11h2v2h-2z'], repere:true },
  star: { d:['M12 3l3 6 6 1-4 5 1 6-6-3-6 3 1-6-4-5 6-1z'], repere:true },
  work: { d:['M3 8h18v11H3z','M9 8V5h6v3'], repere:true },
  volunteer_activism: { d:['M12 9S9 6 7 6a3 3 0 0 0 0 6l5 4','M12 9s3-3 5-3a3 3 0 0 1 0 6l-5 4','M3 14v7h4l6 2 8-4v-2h-8'] },
  // Le Mode organisé (PLAN-MODE-ORGANISE, décision D9) : les dessins
  // du prototype entrent tels quels — planche spikes/mode-organise/.
  portier: { d:['M12 4a3 3 0 1 0 0 6 3 3 0 1 0 0-6','M5 20v-1a7 7 0 0 1 14 0v1','M9.5 13.5v3l2.5-1.5z','M14.5 13.5v3l-2.5-1.5z'] },
  // RETOURS-13 terrain : le kiosque devient un kiosque À JOURNAUX —
  // auvent festonné (arches), guérite, comptoir. Verdict CE devant la
  // planche de 7 (variante B), remplace le dessin du prototype.
  kiosque: { d:['M3 6h18','M3 6v1.5a1.5 1.5 0 0 0 3 0','M6 7.5a1.5 1.5 0 0 0 3 0','M9 7.5a1.5 1.5 0 0 0 3 0','M12 7.5a1.5 1.5 0 0 0 3 0','M15 7.5a1.5 1.5 0 0 0 3 0','M18 7.5a1.5 1.5 0 0 0 3 0','M21 6v1.5','M5 10v10','M19 10v10','M4 20h16','M5 15h14'] },
  registre: { d:['M6 3h12v18l-3-2-3 2-3-2-3 2z','M9 8h6','M9 12h4'] },
  nettoyage: { d:['M3 8h11a3 3 0 1 0-3-4','M3 12h15a3 3 0 1 1-3 4','M3 16h7a2 2 0 1 1-2 3'] },
  pile: { d:['M4 16l8 4 8-4','M4 12l8 4 8-4','M4 8l8-4 8 4-8 4z'] },
  groupe: { d:['M4 10h16v9H4z','M4 14h5l2 2h2l2-2h5','M7 6h10'] },
  // E2 : les points de suspension des minis ⋯ du Portier — trois
  // disques pleins à l'encre, la forme Material redessinée à la
  // grammaire du jeu (le prototype les portait déjà).
  more_horiz: { pleins:[[5,12,2],[12,12,2],[19,12,2]] },
};

// La marque : hors inventaire — le glyphe du document d'icônes,
// verbatim : trait 2,3 et demi-disque r 3.25 tangent au bord haut.
export const MARQUE = { d:['M4 8h16v9H4z'], trait:2.3, flap:'M8.75 9.15A3.25 3.25 0 0 0 15.25 9.15Z' };

// Les glyphes RÉSERVÉS et le jeu dédié des repères, dérivés du
// catalogue — jamais une seconde liste.
export const RESERVES = Object.entries(JEU).filter(([, g]) => g.r).map(([n]) => n);
export const REPERES = Object.entries(JEU).filter(([, g]) => g.repere).map(([n]) => n);
