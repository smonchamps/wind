// ====================================================================
// Le jeu complet, redessiné dans la grammaire du document « Elements ».
//
//   Grille 24 · trait 2 unités · bouts NETS (butt) · jonctions VIVES
//   (miter) · coordonnées entières · aucune correction optique.
//
// Un glyphe par entrée de l'inventaire vendorisé (assets/icones/README.md,
// 78 glyphes). Ce fichier est la SOURCE : la planche (planche.html) et le
// banc (chiffrage.mjs) le lisent tous les deux — on ne mesure pas un
// dessin différent de celui qu'on montre.
//
//   c  classement de coût :
//      'direct'    — la grammaire suffit, aucun arbitrage
//      'arbitrage' — il a fallu décider (une courbe, une diagonale, une
//                    réduction) ; le dessin tient, la décision est à valider
//      'dur'       — la grammaire ne le porte pas à la taille d'emploi ;
//                    dessiné quand même, mais c'est un report, pas un acquis
//   r  réservé au sous-ensemble, employé nulle part (A53/A60/A62)
//   f  famille de FUSION : deux entrées du jeu Material qui, dans cette
//      grammaire, retombent sur le même dessin
// ====================================================================

export const JEU = {
  // --- Boîtes, dossiers, courrier ------------------------------------
  inbox:        { d:['M4 6h16v12H4z','M4 13h5l2 2h2l2-2h5'], c:'direct' },
  all_inbox:    { d:['M4 12h16v7H4z','M6 9h12','M8 6h8'], c:'direct' },
  drafts:       { d:['M4 19V9l8-4 8 4v10z','M4 9l8 4 8-4'], c:'direct',
                  note:"Retour CE (3ᵉ tour) : le triangle du rabat est REDOUBLÉ par symétrie autour de la ligne d'épaules (y = 9) — il forme un losange de sommets (4,9) (12,5) (20,9) (12,13). Aucune cote neuve n'a été décidée : la moitié basse est la moitié haute retournée, le losange sort du dessin qui existait déjà. **Et cela lève la réserve du 2ᵉ tour** — le losange casse la silhouette de toit, le glyphe ne se lit plus comme une maison. Le repli du rabat était la pièce manquante, pas la proportion." },
  archive:      { d:['M3 5h18v4H3z','M5 9h14v10H5z','M9 13h6'], c:'direct', f:'archives' },
  inventory_2:  { d:['M3 4h18v5H3z','M5 9h14v11H5z','M10 6h4'], c:'direct', f:'archives',
                  note:"Même sens qu'`archive` (le dossier contre l'action) : deux dessins pour un sens, la grammaire pousse à fusionner." },
  delete:       { d:['M4 7h16','M6 7v12h12V7','M10 4h4','M10 11v5','M14 11v5'], c:'direct',
                  note:'Retour CE : le corps porte ses deux traits — la poubelle vide se lisait comme une simple boîte.' },
  report:       { d:['M9 3h6l6 6v6l-6 6H9l-6-6V9z','M12 8v5','M12 16v2'], c:'direct',
                  note:"Retour CE : OCTOGONE, comme au Système. Effet de bord mesurable — `report` ne fusionne plus avec `warning`, la famille « triangle » disparaît du chiffrage." },
  warning:      { d:['M12 4l9 16H3z','M12 10v5'], c:'direct',
                  note:'Le triangle lui reste en propre depuis que `report` a repris son octogone : plus de collision.' },
  send:         { d:['M3 4l18 8-18 8 3-8z'], c:'direct',
                  note:"Retour CE : la silhouette double-triangle du Système — un seul chemin fermé, quatre sommets entiers, l'encoche fait le second triangle. La flèche est morte." },
  mark_email_unread:{ d:['M3 8h14v10H3z','M3 8l7 5 7-5'], disque:[19,6,3.4], c:'direct',
                  note:"Retour CE : rabat FERMÉ sur l'enveloppe. Le disque teal reste le seul du jeu à dire un état — c'est sa fonction." },
  forum:        { d:['M6 5V2h16v10h-6','M2 5h14v9H7l-5 5z'], c:'direct',
                  note:"Retour CE : le SECOND message derrière, comme au Système. Seule la part visible du bulle arrière est tracée — le recouvrement se dessine, il ne se superpose pas : deux traits ne passent jamais l'un sur l'autre." },
  description:  { d:['M6 3h8l4 4v14H6z','M9 12h6','M9 16h4'], c:'direct' },
  bookmark:     { d:['M7 3h10v18l-5-5-5 5z'], c:'direct' },

  // --- Écriture -------------------------------------------------------
  edit_note:    { d:['M4 6h10','M4 11h7','M11 19l1-3 6-6 2 2-6 6z'], c:'arbitrage',
                  note:'Le crayon impose une diagonale — le document n’en emploie que dans Helios et le rabat de Moon.' },
  edit_square:  { d:['M4 6h8','M4 6v14h14v-8','M11 15l1-3 6-6 2 2-6 6z'], c:'arbitrage' },
  signature:    { d:['M3 12l4 4','M7 12l-4 4',
                     'M10 17V7h5v6h-5','M17 17v-6h5v6',
                     'M3 21h4','M10 21h4','M17 21h4'], c:'arbitrage',
                  note:"Retour CE (2ᵉ tour) : proposition NEUVE. Le zigzag est mort — il se lisait comme une chaîne de montagnes. À la place, la structure du Système : le x à gauche, deux caractères, la ligne POINTILLÉE dessous. Les deux caractères sont des lettres angulaires (une hampe à panse, une cuvette) : la grammaire n'a pas de boucle, elle a des angles vifs — c'est une évocation d'écriture, pas une imitation de cursive. Le pointillé est trois tirets de 4 u au pas de 7, de x = 3 à x = 21 : symétrique autour de 12. Tout pose sur la même ligne de base, y = 17." },

  // --- Actions de lecture ---------------------------------------------
  reply:        { d:['M9 6L3 12l6 6','M3 12h11a6 6 0 0 1 6 6v1'], c:'arbitrage', arc:true,
                  note:'Sert aussi « Transférer » en symétrie verticale (A12) — aucun glyphe supplémentaire.' },
  reply_all:    { d:['M8 6L2 12l6 6','M14 6l-6 6 6 6','M8 12h8a5 5 0 0 1 5 5v2'], c:'arbitrage', arc:true },
  attach_file:  { d:['M19 7v9a6 6 0 0 1-12 0V6a4 4 0 0 1 8 0v10a2 2 0 0 1-4 0V8'], c:'arbitrage', arc:true,
                  note:"Retour CE (2ᵉ tour) : le repli REVIENT — le trait unique se lisait comme un U, la réserve était fondée. Ce qui a servi la première tentative est gardé : les quatre montants sont à 7, 11, 15 et 19, donc **trois intervalles de 4 unités exactement**. La version d'origine avait 4 u puis 2 u ; c'est la paire à 2 u qui se collait à 16 px, pas le repli lui-même. Trois arcs, trois demi-cercles vrais (corde = 2 r) : aucun rayon décidé à l'œil." },
  keep:         { d:['M10 3h4v7l2 4H8l2-4z','M12 14v7'], c:'direct' },
  keep_off:     { d:['M10 3h4v7l2 4H8l2-4z','M12 14v7','M4 4l16 16'], c:'direct' },
  open_in_full: { d:['M14 4h6v6','M10 20H4v-6','M20 4l-7 7','M4 20l7-7'], c:'direct' },
  open_in_new:  { d:['M14 4h6v6','M20 4l-8 8','M18 13v6H5V6h6'], c:'direct', r:true },
  unfold_more:  { d:['M7 10l5-5 5 5','M7 14l5 5 5-5'], c:'direct' },
  unfold_less:  { d:['M7 5l5 5 5-5','M7 19l5-5 5 5'], c:'direct',
                  note:"Retour CE : les deux pointes s'écartent de 2 à 4 unités — le même écart que `unfold_more`, qui l'avait déjà. Les deux glyphes de la bascule sont enfin cotés pareil." },
  download:     { d:['M12 4v10','M8 11l4 4 4-4','M4 19h16'], c:'direct', f:'fleche-bas' },
  system_update_alt:{ d:['M4 4h16v3H4z','M12 9v8','M8 14l4 4 4-4','M4 21h16'], c:'direct', f:'fleche-bas',
                  note:'Le bandeau supérieur est tout ce qui le sépare de `download`.' },
  visibility_off:{ d:['M3 12l4-4h10l4 4-4 4H7z','M4 4l16 16'], c:'arbitrage',
                  note:"La lentille perd sa pupille : une pupille est un DISQUE, et le disque est réservé à l'état." },
  arrow_back:   { d:['M20 12H4','M10 6l-6 6 6 6'], c:'direct' },
  close:        { d:['M5 5l14 14','M19 5L5 19'], c:'direct' },
  menu:         { d:['M4 7h16','M4 12h16','M4 17h16'], c:'direct' },
  search:       { d:['M11 4a7 7 0 1 0 0 14 7 7 0 1 0 0-14','M16 16l4 4'], c:'arbitrage', arc:true,
                  note:'Cercle de CONTOUR : il ne se lit pas comme un disque, mais il en approche — première tension du jeu.' },
  sync:         { d:['M20 12a8 8 0 1 1-2.3-5.7','M20 4v5h-5'], c:'arbitrage', arc:true },

  // --- Réponses, états ------------------------------------------------
  check_circle: { d:['M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16','M8 12l3 3 5-5'], c:'arbitrage', arc:true, f:'rond-etat' },
  cancel:       { d:['M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16','M9 9l6 6','M15 9l-6 6'], c:'arbitrage', arc:true, f:'rond-etat' },
  error:        { d:['M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16','M12 7v6','M12 15v2'], c:'arbitrage', arc:true, f:'rond-etat',
                  note:"Retour CE : c'est `info` RETOURNÉ, au sens strict — la barre et la marque échangent leurs places par symétrie autour du centre du cercle (y = 12). Deux glyphes, une seule cote." },
  info:         { d:['M12 4a8 8 0 1 0 0 16 8 8 0 1 0 0-16','M12 11v6','M12 7v2'], c:'arbitrage', arc:true, f:'rond-etat',
                  note:"Le point de l'i devient une BARRE de 2 u : un point serait un disque." },
  question_mark:{ d:['M8 8a4 4 0 1 1 4 4v3','M12 18v2'], c:'arbitrage', arc:true },
  priority_high:{ d:['M12 4v10','M12 17v3'], c:'direct' },
  hourglass_empty:{ d:['M6 3h12l-6 9 6 9H6l6-9z'], c:'direct',
                  note:'Un seul sous-chemin, huit nœuds, tout en droites : le glyphe le plus « Elements » de tout le jeu.' },
  schedule_send:{ d:['M9 3a6 6 0 1 0 0 12 6 6 0 1 0 0-12','M9 6v3l2 2'], remplis:['M14 15l8 4-8 4 2-4z'], c:'dur', arc:true,
                  note:"Retour CE : la flèche cède la place au TRIANGLE PLEIN de `send`, même silhouette à l'encoche. Deux sous-chemins au lieu de quatre, et plus rien ne déborde du cadre (l'ancien chevron sortait à y = 26). Reste « dur » : une horloge et un envoi dans 24 unités." },
  link:         { d:['M10 8H8a4 4 0 0 0 0 8h2','M14 8h2a4 4 0 0 1 0 8h-2','M9 12h6'], c:'arbitrage', arc:true, r:true },
  link_off:     { d:['M10 8H8a4 4 0 0 0 0 8h2','M14 8h2a4 4 0 0 1 0 8h-2','M4 4l16 16'], c:'arbitrage', arc:true },
  notifications:{ d:['M7 17v-6a5 5 0 0 1 10 0v6','M4 17h16','M10 20h4'], c:'arbitrage', arc:true },

  // --- Personnes -------------------------------------------------------
  person:       { d:['M12 5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 1 0 0-7','M5 20v-1a7 7 0 0 1 14 0v1'], c:'arbitrage', arc:true,
                  note:'La tête est un cercle plein chez Material ; ici de contour, sinon elle devient un disque de 7 u.' },
  person_add:   { d:['M9 5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 1 0 0-7','M2 20v-1a7 7 0 0 1 14 0v1','M19 4v6','M16 7h6'], c:'dur', arc:true },
  group_add:    { d:['M8 5a3 3 0 1 0 0 6 3 3 0 1 0 0-6','M2 19v-1a6 6 0 0 1 12 0v1','M19 4v6','M16 7h6'], c:'dur', arc:true },

  // --- Réglages ---------------------------------------------------------
  settings:     { d:['M4 8h16','M14 5v6','M4 16h16','M9 13v6'], c:'direct',
                  note:'La roue dentée est intransposable (12 dents à 16 px) : deux glissières la remplacent.' },
  display_settings:{ d:['M3 5h18v12H3z','M6 11h12','M10 9v4','M9 20h6'], c:'arbitrage' },
  keyboard:     { d:['M3 7h18v10H3z','M7 14h10','M7 10h2','M11 10h2','M15 10h2'], c:'dur',
                  note:'Trois touches de 2 u dans un cadre : à 16 px elles fusionnent avec le cadre.' },

  // --- Barre de mise en forme (A62) --------------------------------------
  format_bold:  { d:['M8 4v16','M8 4h6l3 4-3 4H8','M8 12h7l3 4-3 4H8'], c:'direct' },
  format_italic:{ d:['M10 4h8','M6 20h8','M14 4l-4 16'], c:'direct' },
  format_underlined:{ d:['M7 4v8l5 5 5-5V4','M5 20h14'], c:'direct' },
  strikethrough_s:{ d:['M16 7h-6l-2 3 8 4-2 3H8','M4 12h16'], c:'arbitrage',
                  note:"L'S est rendu en zigzag mitré — une lettre dans une grammaire qui n'en a pas." },
  format_color_text:{ d:['M6 15l6-11 6 11','M9 11h6'], barre:'M4 19h16', c:'arbitrage',
                  note:'La barre du bas est le SEUL élément coloré légitime du jeu : elle porte la couleur choisie, elle ne décore pas.' },
  format_align_left:  { d:['M4 6h16','M4 10h10','M4 14h16','M4 18h10'], c:'direct' },
  format_align_center:{ d:['M4 6h16','M7 10h10','M4 14h16','M7 18h10'], c:'direct' },
  format_align_right: { d:['M4 6h16','M10 10h10','M4 14h16','M10 18h10'], c:'direct' },
  format_list_bulleted:{ d:['M4 6h2','M4 12h2','M4 18h2','M10 6h10','M10 12h10','M10 18h10'], c:'direct',
                  note:'Puces CARRÉES (barres de 2 u) : des puces rondes seraient trois disques.' },
  format_list_numbered:{ d:['M11 5h10','M11 12h10','M11 19h10',
                            'M5 2v6','M3 8h4',
                            'M2 9h5v3H2v3h5',
                            'M2 16h5v3H3','M7 19v3H2'], c:'dur',
                  note:"Retour CE (2ᵉ tour) : REPRIS. Les chiffres passent de 4 à 6 unités de haut et de 3 à 5 de large, les lignes reculent de x = 10 à x = 11 pour leur céder la place, et chacun se centre sur SA ligne (5, 12, 19). À 16 px un chiffre gagne de 2,7 à 4,0 px de haut. Reste « dur », et c'est le verdict honnête : trois chiffres dans une colonne de 5 unités ne seront jamais lisibles à 16 px, quelle que soit la grammaire. C'est le glyphe qui plaide le plus fort pour le palier 16 dessiné à la main." },
  format_indent_increase:{ d:['M4 6h16','M11 10h9','M11 14h9','M4 18h16','M5 10l3 2-3 2'], c:'arbitrage' },
  format_indent_decrease:{ d:['M4 6h16','M11 10h9','M11 14h9','M4 18h16','M8 10l-3 2 3 2'], c:'arbitrage' },
  format_clear: { d:['M3 5h12v5H3z','M15 7h4v6h-7v8','M2 2l20 20'], c:'arbitrage',
                  note:"Retour CE (2ᵉ tour) : la barre va de coin à coin (2,2 → 22,22) au lieu de s'arrêter au ras du rouleau. Une barre qui commence sur ce qu'elle barre ne barre rien — elle doit entrer et sortir de la figure." },
  format_quote: { d:['M5 14V9h5v5l-3 4H5z','M14 14V9h5v5l-3 4h-2z'], c:'direct', r:true },

  // --- Réservés ----------------------------------------------------------
  storage:      { d:['M3 5h18v4H3z','M3 10h18v4H3z','M3 15h18v4H3z'], c:'direct', r:true },

  // --- Les 12 repères de compte (A74, PLAN-RETOURS-8 R1) ------------------
  // Rendus à 10-12 px DANS une pastille. Sous cette direction, le compte
  // est un DISQUE NU : ces douze glyphes disparaissent — ou bien la
  // direction plie. Dessinés ici pour que le coût des deux branches soit
  // visible, et non pour être adoptés en l'état.
  account_balance:{ d:['M3 9l9-5 9 5','M5 9v9','M10 9v9','M14 9v9','M19 9v9','M3 20h18'], c:'dur', repere:true },
  eco:          { d:['M5 19L19 5','M5 19c0-9 6-14 15-14 0 9-6 14-15 14z'], c:'dur', arc:true, repere:true },
  favorite:     { d:['M12 20S4 14 4 9a4 4 0 0 1 8-1 4 4 0 0 1 8 1c0 5-8 11-8 11z'], c:'dur', arc:true, repere:true },
  flight:       { d:['M11 3h2l1 8 7 3v2l-7-1-1 5 3 2v1l-4-1-4 1v-1l3-2-1-5-7 1v-2l7-3z'], c:'dur', repere:true },
  home:         { d:['M3 11l9-7 9 7','M6 10v10h12V10'], c:'direct', repere:true },
  music_note:   { d:['M11 18V6','M11 6l5 3'], pleins:[[8, 18, 3]], c:'arbitrage', repere:true,
                  note:"Retour CE : une note SEULE — hampe, disque plein en bas, petit trait diagonal en haut. On passe de deux têtes à une, et de deux arcs à zéro. La tête est tangente à la hampe, pas posée à côté." },
  pets:         { d:['M12 21a4 4 0 0 1-4-4c0-3 8-3 8 0a4 4 0 0 1-4 4z'],
                  pleins:[[4, 10, 2], [9, 6, 2], [15, 6, 2], [20, 10, 2]], c:'dur', arc:true, repere:true,
                  note:"Retour CE (3ᵉ tour) : les deux coussinets du haut passent de 4 à 6 unités de centre à centre — à 4, avec r = 2, ils étaient exactement TANGENTS, ce qui les soudait en une seule masse. Les extérieurs s'écartent d'autant (4 et 20) : les trois cordes tombent à 6,4 / 6,0 / 6,4, et plus aucune paire ne se touche. Disposition du Système, tenue au 2ᵉ tour ; l'espacement identique du 1ᵉʳ reste abandonné — quatre centres entiers sur un arc ne peuvent pas tenir trois cordes égales. Reste quatre disques dans un système qui réserve le rond à l'état : incompatible avec la doctrine, pas seulement cher." },
  school:       { d:['M2 9l10-4 10 4-10 4z','M6 12v5c0 2 12 2 12 0v-5'], c:'dur', arc:true, repere:true },
  shopping_bag: { d:['M5 7h14v13H5z','M9 7V5a3 3 0 0 1 6 0v2'], c:'arbitrage', arc:true, repere:true },
  sports_esports:{ d:['M4 16l2-7h12l2 7a2 2 0 0 1-3 1l-2-2H9l-2 2a2 2 0 0 1-3-1z'],
                  remplis:['M7 11h2v2H7z','M15 11h2v2h-2z'], c:'dur', arc:true, repere:true,
                  note:"Retour CE (3ᵉ tour) : les carrés deviennent PLEINS au lieu d'être tracés. Un carré de 2 u tracé au trait de 2 u rendait un pavé de 4 u qui touchait le boîtier ; plein, il fait vraiment 2 u et garde ~1 unité de dégagement sur les quatre bords. Même taille tous les deux, centres (8,12) et (16,12) — symétriques autour de x = 12." },
  star:         { d:['M12 3l3 6 6 1-4 5 1 6-6-3-6 3 1-6-4-5 6-1z'], c:'dur', repere:true },
  work:         { d:['M3 8h18v11H3z','M9 8V5h6v3'], c:'direct', repere:true },
  volunteer_activism:{ d:['M12 9S9 6 7 6a3 3 0 0 0 0 6l5 4','M12 9s3-3 5-3a3 3 0 0 1 0 6l-5 4','M3 14v7h4l6 2 8-4v-2h-8'], c:'dur', arc:true },
};

// La marque : hors inventaire. C'est le glyphe du document, verbatim —
// trait 2,3 et demi-disque r 3.25 tangent au bord intérieur haut.
export const MARQUE = { d:['M4 8h16v9H4z'], trait:2.3, flap:'M8.75 9.15A3.25 3.25 0 0 0 15.25 9.15Z' };

// Les glyphes RÉSERVÉS au sous-ensemble mais employés nulle part.
export const RESERVES = Object.entries(JEU).filter(([, g]) => g.r).map(([n]) => n);
// Les glyphes du jeu dédié aux repères de compte.
export const REPERES = Object.entries(JEU).filter(([, g]) => g.repere).map(([n]) => n);
