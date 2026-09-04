> **Historical record — French, frozen** (closed on 2026-08-25; PLAN-ENGLISH-SWITCH
> D1, debt D-58). Not translated; the living documentation is in `docs/`.

# PLAN-REPERE-LIGNE — la boîte se dit en toutes lettres

> **Chantier OUVERT et IMPLÉMENTÉ — en attente du verdict terrain
> (STOP 2).** Il porte le verdict
> du Chef Ingénieur sur l'exploration du 2026-08-24 (22 dessins comparés,
> 5 planches jetables dans `spikes/volet-repere/`), les amendements
> écrits au Système, la spécification d'implémentation, et les trouvailles
> annexes de l'exploration.
> **Les dix décisions CE sont prises** (§2, 2026-08-24) : le STOP 1 est
> passé.
>
> **SOLDÉ au terrain le 2026-08-25** — les cinq étapes E1-E5 livrées,
> revue à regard neuf passée (8 angles, 40 candidats, 14 défauts
> distincts confirmés — tous corrigés, §9), **gate complète VERTE**
> (9/9 étapes, e2e 124 → **129**), **terrain CE 15/15** : quatorze
> points OK à la première passe, **un seul constat — le point 12** :
> le volet de lecture disait encore la boîte dans la vue d'un seul
> compte, alors que la liste se taisait. Corrigé le jour même, dans la
> même session (§10), re-gate verte, **point 12 revalidé OK**.

> Énoncé d'origine (2026-08-24) : « retirer la vignette avec les
> initiales des emails, et rendre à la fois visible et discrète la
> vignette de la boîte sur laquelle a été reçu l'email ». Trois passes
> d'exploration : 7 dessins de marque, 15 organisations de volet, 7
> formes de ligne d'expéditeur. Verdict : **la boîte se dit en toutes
> lettres, sur la ligne de l'expéditeur**.

---

## 1. Ce que l'exploration a établi — faits mesurés

Tous les chiffres ci-dessous sont **mesurés au rendu** des planches
(`spikes/volet-repere/`), sur les jetons et les glyphes **lus du
produit** (`systeme.css`, `lib/icones.js`) — jamais recopiés.

### 1.1 La forme retenue

```
[disque non-lu] Camille Roux  sur ▣ Travail            09:41
Contrat Vantis — v4 pour relecture
Voici la version corrigée, les deux annexes sont à jour.
[📎 2 fichiers]
```

- Le bloc boîte = **`sur`** (encre `--muted`) + **le glyphe du repère**
  (14 px, à la teinte du compte, sans contenant) + **le libellé**
  (encre `--ink2`).
- Il vit **dans la ligne d'entête**, entre le nom d'expéditeur et
  l'heure. Aucun rang neuf : **les deux gabarits d'A44 ne bougent pas**
  (88 / 115 px mesurés, identiques à aujourd'hui).
- La **tuile aux initiales de l'expéditeur disparaît de la liste**. Elle
  reste au fil et au dossier Brouillons (§2, D9).

### 1.2 Les chiffres qui ont tranché

| | |
|---|---|
| **Le bloc coûte** | 83 px sur une ligne qui en offre 365 au défaut (volet 400 px). |
| **Place restante au nom** | 219 px au défaut, contre 304 px sans le bloc. **Aucun nom d'expéditeur n'est coupé** sur les 14 rangées du décor, « Bibliothèque universitaire » comprise. |
| **À la borne basse (300 px)** | 116 px au nom, 3 noms coupés sur 14. La ligne ne déborde jamais. |
| **Nom de compte long** (32 caractères) | Le libellé se tronque à l'ellipse ; **0 nom d'expéditeur coupé au défaut**. |
| **Le glyphe nu, contraste** | Pire cas du nuancier entier sur `bg` / `hover` / `sel` / `tuile`, deux polarités : **4,97:1** — bien au-dessus du seuil composant de 3:1. **Aucune paire neuve à mesurer.** |
| **Formes rondes restantes** dans la fenêtre entière | **`.disque` seulement** (non-lu de rangée, barre d'état) — 20 éléments comptés, zéro pastille. |

### 1.3 Le plafond du bloc — pourquoi le tiers

Six plafonds essayés sur un compte nommé « Association des parents
d'élèves » (32 caractères ; D4 en accepte 60), aux trois largeurs :

| plafond | volet 400, nom long | volet 300, nom long |
|---|---|---|
| 50 % | bloc 183 px — **2 expéditeurs coupés** | 3 coupés |
| 42 % | bloc 153 px — 1 coupé | 3 coupés |
| **33 % — le tiers** | bloc 120 px — **0 coupé** | 3 coupés, **libellés courts intacts** |
| 30 % | bloc 110 px — 0 coupé | **7 libellés coupés**, dont des courts pour rien |

33, 34, 35 et 36 % donnent le même résultat : c'est un **plateau**, pas
une valeur de justesse — d'où le tiers, qui se dit en un mot. **Coût
assumé** : un nom de 32 caractères reste tronqué même à 640 px. La boîte
est une circonstance ; elle ne prend pas le tiers d'une rangée quelle
que soit la largeur.

---

## 2. Décisions CE — toutes tranchées le 2026-08-24

| | | |
|---|---|---|
| **D1** | **La forme est la phrase.** « Expéditeur sur ▣ Libellé ». | Motif du CE : elle se lit, et elle évite d'avoir à se souvenir en permanence d'une couleur ou d'un logo. |
| **D2** | **Le glyphe est le même des deux côtés.** | Que la nav et la ligne disent la même chose n'est pas choquant ; le glyphe, lui, doit être exactement le même objet. |
| **D3** | **Le glyphe reste.** | Il donne de la chaleur et une humanité discrète ; couvrir couleur **et** forme couvre la majorité des goûts pour une implémentation simple. |
| **D4** | **La troncature, pas le repli.** Le libellé se tronque à l'ellipse quand il s'approche de la date / l'heure. | Écarte le repli au seuil (V7 de l'exploration) : un libellé qui disparaît d'un coup surprend, là où une ellipse dit ce qu'elle fait. Plafond au **tiers**, mesuré (§1.3). |
| **D5** | **Le même schéma au volet de lecture**, derrière le nom de l'expéditeur. | Carte dépliée **et** rangées repliées. |
| **D6** | **Le glyphe NU dans la nav.** La pastille pleine quitte l'écran 02. | Conséquence de D2 : les deux surfaces portent le même objet. |
| **D7** | **Le bloc ne vit QUE là où les comptes se mélangent** — boîte unifiée et recherche. | D3 d'A74 hérité tel quel. Dans la vue d'un seul compte, « sur Travail » sur chaque rangée n'apprend rien ; et le bloc étant **en ligne**, son absence ne décale rien — aucune colonne n'est réservée. |
| **D8** | **Un compte sans repère dit quand même sa boîte** : le bloc s'affiche, sans glyphe. Libellé = **nom personnalisé (A78) si posé, sinon l'adresse**. | C'est le gain propre de la forme en lettres : le mot suffit, et deux comptes sans repère cessent d'être indiscernables (§6.3). L'adresse sera tronquée au tiers ; l'infobulle la donne entière. |
| **D9** | **La tuile aux initiales : retrait de la LISTE SEULE.** | Elle vit encore aux cartes du fil (`Fil.svelte:264`, `:428`) — l'expéditeur y change d'un message à l'autre — et au dossier Brouillons (`Liste.svelte:888`), où elle dit le destinataire. À écrire au Système, sinon c'est une incohérence muette. |
| **D10** | **Le mot anglais est `in`** — « Camille Roux in Work ». | La boîte est un contenant, pas un support. |

**Non-but explicite** : rien dans ce chantier ne touche au tri, au
regroupement, au bandeau ni au pied. Les 15 organisations explorées
(`organisation.html`) restent au dossier, écartées ou reportées.

---

## 3. Amendements au Système

> **Le journal ne se réécrit pas** — c'est une archive de faits datés
> (`docs/design/systeme.dc.html`, § Journal des amendements). Les
> entrées V4, V5, V14 et A74 restent **verbatim**. Ce qui change, c'est
> le **corps** du document (les sections normatives), et **trois entrées
> neuves** s'ajoutent à la suite d'A79.

### 3.1 Les trois entrées à ajouter au journal

À insérer dans `<tbody>` de la table « Journal des amendements », après
la ligne `A79`, en gardant la forme `<tr><td class="nw">date</td><th
scope="row">An</th><td>…</td></tr>`.

**A80 — La boîte se dit en toutes lettres, sur la ligne de
l'expéditeur.** (PLAN-REPERE-LIGNE ; décisions CE D1-D5 du 2026-08-24.)
Le badge de repère sous l'avatar (A74) est remplacé par un **bloc de
texte** dans la ligne d'entête : `sur` à l'encre atténuée, le glyphe du
repère à la teinte du compte, puis le **libellé de la boîte** — nom
personnalisé (A78) ou adresse. Motif du Chef Ingénieur : *la phrase se
lit, elle évite d'avoir à se souvenir en permanence d'une couleur ou
d'un logo.* Trois règles de troncature, et elles se disent chacune en
une phrase : **l'heure ne se coupe jamais** (c'est le repère de lecture
de la colonne) ; **le bloc boîte cède trois fois plus vite que
l'expéditeur et ne prend jamais plus du tiers de la ligne** ; **les deux
se terminent à l'ellipse**, jamais à la coupe sèche. Le tiers est
mesuré, pas choisi : sur un nom de 32 caractères, à la moitié deux noms
d'expéditeur se coupent au défaut, à 30 % ce sont les libellés **courts**
qui se coupent pour rien à la borne basse (7 sur 16) ; 33 à 36 % donnent
le même résultat. **A8 s'en trouve renforcé** : l'information est portée
par un MOT, la couleur et le glyphe ne font que la doubler. Le bloc vit
là où les comptes se mélangent (D3 d'A74, inchangé), et **le même bloc se
répète au volet de lecture**, derrière le nom de l'expéditeur (D5).
Aucun rang neuf, aucun glyphe neuf, **aucune paire de contraste neuve** ;
les deux gabarits d'A44 sont mesurés inchangés (88 / 115 px).

**A81 — La tuile aux initiales quitte la liste.** (PLAN-REPERE-LIGNE ;
décision CE D9 du 2026-08-24.) La colonne de tête de la rangée disparaît :
les initiales de l'expéditeur ne disaient rien que le nom, écrit en
toutes lettres à 10 px au-dessus, ne disait déjà — et elles coûtaient
38 px sur les 400 du volet, au bénéfice de l'objet qui, lui, apprenait
quelque chose. **La tuile survit là où elle travaille** : les cartes du
fil (l'expéditeur y change d'un message à l'autre) et le dossier
Brouillons (elle y dit le destinataire). Conséquence sur la phrase de
V4 — « l'avatar d'initiales devient un carré » — : elle reste vraie de
ses deux emplois restants, et cesse de valoir pour la rangée de liste.

**A82 — Le repère est un glyphe nu ; la pastille se retire aux
Réglages.** (PLAN-REPERE-LIGNE ; décision CE D6 du 2026-08-24.) La
pastille pleine de 20 px quitte la navigation : le repère s'y trace **à
même le fond**, à la teinte du compte, **16 px** — la taille des glyphes
de dossier de la même colonne, si bien que les rangées de comptes cessent
d'être des rangées à part. Motif : la nav et la ligne de liste doivent
porter **exactement le même objet** (D2). Ce que le Système perd : la
phrase de V4/V14 « reste un seul autre rond dans tout le système, la
pastille de repère » cesse de valoir pour l'écran 02 — **mesuré sur la
fenêtre entière, la seule forme ronde restante est le disque**
(non-lu de rangée, barre d'état). Le disque ne dit donc plus **que**
l'état, ce que V4 visait sans l'atteindre. Ce que le Système garde : la
pastille vit aux **Réglages** (rangée de compte et nuancier de choix),
où elle est une pastille **de choix** et non une marque d'identité — V5
tient donc entièrement dans son domaine restant. Contrastes : la teinte
en **tracé** se pose sur les mêmes fonds que la pastille et la table de
mesure ne bouge pas (pire cas du nuancier : 4,97:1, seuil composant 3:1).
Ce qui est perdu, et dit : le fond coloré donnait au repère une présence
à distance ; un tracé de 2 unités à 16 px pèse 1,3 px — la nav dit le
compte plus doucement.

### 3.2 Les passages du corps à réécrire

| # | où | ce qui est écrit | ce qui doit l'être |
|---|---|---|---|
| 1 | `systeme.dc.html:1521-1522`, fiche « Le disque dit l'état — et rien d'autre (V4) » | « Reste un **seul autre rond** dans tout le système : la pastille de repère de compte, Ø 16 px — et elle porte un glyphe… » | Le disque est **la seule forme ronde de l'écran 02** (A82). La pastille de repère subsiste **aux Réglages** — pastille de choix, pas marque d'identité ; elle y garde son glyphe pour la raison de V5. |
| 2 | `systeme.dc.html:~1490`, fiche « Zéro rayon — et deux formes qui n'en sont pas » | « le disque (50 %) dit l'**état et l'identité** — non-lu, cycle, **repère de compte**, poignée d'interrupteur » | Retirer « repère de compte » de la liste : dans le produit, le disque dit l'**état** (non-lu, cycle) et la **poignée d'interrupteur**. L'identité se dit désormais en lettres (A80) et en tracé (A82). |
| 3 | `systeme.dc.html:~3940`, § « Règles de transmission » (ligne de message) | La prose décrit la rangée avec sa tuile d'initiales et le badge sous l'avatar. | Réécrire la rangée : plus de colonne de tête ; la ligne d'entête porte disque de non-lu, nom d'expéditeur, **bloc de boîte**, heure. Y poser les **trois règles de troncature** d'A80. La maquette de la section doit être redessinée en conséquence. |
| 4 | `systeme.dc.html:~1252` et `~2329`, nuancier et jeu dédié des repères | « les deux tailles d'emploi de la pastille : 20 px (nav, Réglages) et 16 px (badge de liste) » | **Une** taille de pastille : 20 px, **aux Réglages seulement**. Ajouter l'emploi en **tracé** : 16 px dans la nav, 14 px dans la ligne d'entête et au volet de lecture. |
| 5 | `systeme.dc.html:~2065`, relevé des icônes, entrée `person` | « Boîte de compte et tuile de la boîte en cours — le DÉFAUT d'un compte sans repère ; un repère choisi le remplace par sa **pastille** (A74) » | … le remplace par son **tracé** (A82). |
| 6 | § « Écran 02 », volet de lecture | La carte de message dit « nom / adresse · à destinataire ». | Ajouter le bloc de boîte derrière le nom, sur la carte dépliée **et** sur les rangées repliées (D5). |

**À ne PAS toucher** : les entrées V4, V5, V14, A74, A78 du journal —
elles sont datées et vraies de leur date. A80-A82 disent ce qu'elles
renversent ; c'est le rôle du journal.

---

## 4. Spécification d'implémentation

### 4.1 Le bloc de boîte — l'objet partagé

Il est **posé à trois endroits** (liste, carte de fil dépliée, rangée
repliée du fil) : il vit donc dans `systeme.css`, pas dans un composant.
Une seule implémentation, comme la pastille.

```html
<span class="boite" title="{libelle} — {adresse}">
  <span class="mot">sur</span>
  <!-- si le compte porte un repère (A74) : -->
  <span class="repere-nu" data-teinte="{teinte}" aria-hidden="true">
    <Icone nom={icone} taille={14} /></span>
  <span class="lib">{libelle}</span>
</span>
```

```css
/* A80 — le bloc de boîte : « sur <tracé> Libellé ». Partagé liste/fil.
   L'ORDRE DE TRONCATURE EST LE DESSIN : l'heure ne cède jamais, le bloc
   cède trois fois plus vite que l'expéditeur, et le tiers est un
   plafond mesuré (voir PLAN-REPERE-LIGNE §1.3). La préposition et le
   tracé ne rétrécissent pas : un « sur » tronqué ne dirait rien. */
.boite {
  flex:0 3 auto; min-width:0; max-width:33%;
  display:inline-flex; align-items:center; gap:5px;
  font-size:13px; color:var(--ink2); white-space:nowrap;
}
.boite .mot { color:var(--muted); flex:none; }
.boite .repere-nu { flex:none; }
.boite .lib { min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
/* Une rangée non lue met la graisse sur ce qu'elle DIT, pas sur ses
   circonstances (A8/V6) : le bloc reste en graisse normale. */
.nonlu .boite { font-weight:400; }

/* A82 — le repère NU : le tracé du repère, à la teinte du compte, sans
   contenant. Deux tailles d'emploi : 16 px (nav, celle des glyphes de
   dossier) et 14 px (ligne d'entête, celle de son texte). */
.repere-nu { display:inline-flex; flex:none; }
```

### 4.2 Le nuancier — une seule table de hex, et le piège de la gate

Les 24 hex servent désormais **deux fois** : en `background` (pastille
des Réglages) et en `color` (tracé). Les recopier serait la faute que le
Système reproche partout ailleurs. **Les poser en jetons** :

```css
:root {
  --rep-rouge:#a93226; --rep-orange:#9c4a06; /* … 12 familles */
}
:root[data-theme="elements-nuit"] {
  --rep-rouge:#f1998e; /* … */
}
.repere[data-teinte="rouge"]    { background:var(--rep-rouge); }
.repere-nu[data-teinte="rouge"] { color:var(--rep-rouge); }
```

⚠️ **Deux pièges, tous deux dans la gate — à traiter dans le MÊME
commit :**

1. `e2e/contraste.mjs` (`lireReperes`) lit les hex par la regex
   `\.repere\[data-teinte="…"\]\s*\{\s*background:(#…)`. Passer aux
   jetons **casse la lecture en silence** : la gate ne trouverait plus
   12 familles et sortirait en échec (c'est le comportement voulu — mais
   il faut amender la regex pour lire `--rep-<teinte>` dans les blocs de
   thème, et **garder le compte de 12 × 2**).
2. `e2e/jetons.mjs` (`lireThemes`) capture `--([a-zA-Z][a-zA-Z0-9]*)` :
   la classe **ne contient pas le trait d'union**, donc `--rep-rouge`
   échappe au parseur et ne gonfle pas la table des 17 jetons. **C'est
   une protection accidentelle** : si quelqu'un « corrige » un jour cette
   classe en `[a-zA-Z0-9-]`, le contrôle 1 de `coherence-systeme.mjs`
   tombera sur 12 jetons « livrés mais absents du doc ». À écrire en
   commentaire au-dessus des jetons `--rep-*`.

Le libellé de la mesure dans `contraste.mjs` (« nav, badge de liste »)
devient « **Réglages (pastille), nav et ligne (tracé)** ».

### 4.3 `Liste.svelte`

- **Retirer** `.col-avatar`, `.avatar` et l'import de `initiales` du
  chemin de la rangée (le dossier Brouillons garde le sien — D9).
- La grille passe de `grid-template-columns:auto 1fr` à `1fr` ; les
  quatre blocs (`.l1`, `.objet`, `.apercu`, `.puces`) passent en
  `grid-column:1`.
- `.l1` : `gap` de **10 → 6 px** (le bloc ajoute deux gouttières ; à 10
  la ligne perd 12 px pour rien), et l'ordre devient :

```html
<div class="l1">
  {#if ligne.thread_unseen > 0}<span class="disque"></span>{/if}
  {#if epinglee}<span class="marque-epingle">…</span>{/if}
  <span class="exp">…</span>
  {#if boite}<span class="boite" …>…</span>{/if}
  <span class="essor"></span>
  <span class="heure">{quand(ligne.epoch)}</span>
</div>
```

```css
.exp    { flex:0 1 auto; min-width:0; overflow:hidden;
          text-overflow:ellipsis; white-space:nowrap; }  /* était flex:1 */
.essor  { flex:1 1 0; min-width:0; }                     /* neuf */
.heure  { flex:none; }                                    /* déjà */
```

- Le dérivé qui remplace `repere` (ligne 748) :

```js
// A80 : le bloc vit là où les comptes se MÉLANGENT — boîte unifiée
// (D3 d'A74) et recherche, D7. À la différence du badge, il ne demande
// PAS de repère : le mot suffit, et le libellé retombe sur l'adresse
// quand aucun nom n'est posé (D8).
const boiteDe = (ligne) => {
  if (compte !== null && resultats === null) return null;
  const libelle = noms[ligne.account_id] ?? ligne.account_email;
  return {
    repere: reperes[ligne.account_id] ?? null,
    libelle,
    titre: `${libelle} — ${ligne.account_email}`,
  };
};
```

- `data-testid` : `ligne-repere` → **`ligne-boite`** (la couture change
  de sens : elle ne dit plus un badge mais un bloc, repère ou non).
- Les **sondes** (`h1`/`h2`) doivent perdre leur `.avatar` : les deux
  gabarits sont mesurés inchangés, mais une sonde qui rend un objet mort
  mentirait sur la géométrie.

### 4.4 `Nav.svelte`

- Les deux emplois de `<span class="repere p20">` (lignes 83 et 96)
  deviennent :

```html
<span class="repere-nu" data-teinte={b.repere.teinte} aria-hidden="true">
  <Icone nom={b.repere.icone} taille={16} /></span>
```

- Le repli sans repère (`<span class="icone"><Icone nom="person" /></span>`)
  **ne change pas** — même taille (16), l'encre reste `--muted`.
- `data-testid="nav-repere"` **conservé** : la couture désigne toujours
  « le repère de cette rangée ».

### 4.5 Le volet de lecture (D5) — la plomberie manque

C'est le seul point qui demande du **câblage neuf** : `Fil.svelte` ne
reçoit ni `reperes` ni `noms` aujourd'hui.

- `App.svelte` : passer `{reperes} {noms}` à `<Lecture …>` (ligne **1375**)
  **et** à `<Conversation …>` (ligne **1461**) — le fil vit dans **deux cadres**
  (`Lecture.svelte:42`, `Conversation.svelte:64`), les deux doivent
  servir, sinon l'écran 03 dirait moins que le volet.
- `Lecture.svelte` et `Conversation.svelte` : ajouter les deux props et
  les transmettre à `<Fil>`.
- `Fil.svelte` : le bloc se pose derrière le nom, aux deux endroits —
  la tête de message dépliée (ligne ~266, après `.auteur`) et la rangée
  repliée (ligne ~428, après le nom). Le compte se lit sur le message
  (`m.account_id`), pas sur le fil : c'est l'identité canonique
  (invariant 2 du STANDARD).
- La tuile aux initiales du fil **reste** (D9).
- Graisse : le bloc reste en graisse normale sur la carte — c'est le nom
  qui porte l'autorité.

### 4.6 Catalogue (i18n)

Une clé neuve, dans les deux catalogues, à côté des clés `liste.*` :

```js
'liste.sur': 'sur',   // catalogue.fr.js
'liste.sur': 'in',    // catalogue.en.js — D10 : « Camille Roux in Work »
```

**Limite à dire** : la phrase est assemblée de trois nœuds (nom, bloc,
heure), donc l'ordre des mots est figé dans le dessin. Français et
anglais s'en accommodent ; une troisième langue qui demanderait un autre
ordre exigerait un gabarit, pas une clé. À rouvrir seulement si le cas
se présente.

### 4.7 Accessibilité

- Le tracé est `aria-hidden="true"` : il **double** le mot, il ne
  l'ajoute pas.
- `title` sur le bloc : « Libellé — adresse » (l'adresse reste la vérité
  technique, même quand un nom personnalisé s'affiche).
- La troncature est **CSS seulement** : le libellé entier reste dans le
  DOM, donc lu entièrement par les technologies d'assistance.
- **A8 renforcé** : l'origine est dite par un mot ; couleur et forme la
  doublent. C'est plus fort que ce qu'A74 obtenait, et il faut le dire
  dans A80.

### 4.8 Ce qui ne bouge pas — et qu'il faut vérifier au lieu de croire

- **Le fenêtrage** : le bloc est en ligne, aucun rang neuf ;
  `chipsParPage`, `extraPuce` et la correction itérative d'A44 sont
  intacts. Les gabarits sondés changent de valeur au pixel près (la
  colonne de tête disparaît) mais restent **deux**.
- **Le chemin d'affichage** : aucun appel neuf, aucun comptage.
  `reperes` et `noms` sont déjà chargés par l'App (A64 tenu).
- **La recherche** : elle mélange toujours les comptes, donc le bloc s'y
  affiche — c'est déjà la règle du badge (revue 2026-08-22).
- **Le dossier Envoyés** : la rangée y dit « À : X » (A48). La phrase
  devient « À : Marine Alonso sur ▣ Travail » — à **regarder** dans ce
  dossier précis au terrain, c'est la seule composition non testée par
  les planches.

### 4.9 Ordre de travail — cinq étapes, gate UNE fois à la fin

| | | |
|---|---|---|
| **E1** | Le nuancier en jetons `--rep-*` + `.repere-nu` dans `systeme.css` ; `contraste.mjs` amendé. | RED possible : la gate contraste doit tomber avant, puis repasser à 12 × 2 familles. |
| **E2** | `Nav.svelte` : le glyphe nu (A82). | e2e `refonte-retours-8` amendé. |
| **E3** | `Liste.svelte` : la tuile meurt, le bloc naît (A80, A81), sondes comprises. | e2e neuf + `refonte-ecran02:93` retiré. |
| **E4** | Le volet de lecture : plomberie `reperes`/`noms` → `Fil` (D5), deux cadres. | e2e neuf. |
| **E5** | Le Système : §3.1 et §3.2, en un seul commit avec le code qu'ils décrivent. | `coherence-systeme.mjs` verte. |

Boucle intérieure : les seules specs impactées, en fichier entier ; la
**gate complète une fois** avant le commit (STANDARD §2.4).

---

## 5. Tests

### 5.1 Ce qui meurt

- `e2e/tests/refonte-ecran02.spec.js:93` — « la ligne de liste porte
  l'avatar aux initiales ». **Son objet disparaît.** Les assertions du
  **fil** (`:317`, `:320`, `:349`) restent : la tuile y vit encore (D9).

### 5.2 Ce qui s'amende

- `refonte-retours-8.spec.js:69-91` — la nav : `[data-testid="nav-repere"]`
  survit, mais l'assertion `.repere` (classe) devient `.repere-nu`, et le
  contrôle du glyphe (`data-nom="home"`) reste tel quel.
- `refonte-retours-8.spec.js:97-108` — « le badge de liste ne vit qu'en
  boîte unifiée (D3) » devient « **le bloc de boîte** ne vit qu'en boîte
  unifiée » : `ligne-repere` → `ligne-boite`, et l'assertion change de
  sens — **toutes** les rangées portent le bloc en boîte unifiée (D8),
  plus seulement celles d'un compte à repère. La borne du test
  (`nBadges < nLignes`) devient `nBlocs === nLignes`.
- `refonte-retours-8.spec.js:122-123` — retirer le repère laisse le bloc
  **sans son tracé**, pas sans bloc.

### 5.3 Ce qui naît

1. **La ligne dit la boîte en toutes lettres** : en boîte unifiée, la
   première rangée porte `sur` + le libellé du compte de la ligne.
2. **Un compte sans repère dit quand même sa boîte** (D8) : bloc présent,
   `.repere-nu` absent.
3. **La troncature protège l'heure et le nom** : volet réglé à 300 px
   (`appliquerLargeur('liste', 300)`), l'heure reste visible et le
   libellé long est tronqué (`scrollWidth > clientWidth` sur `.lib`).
4. **Le volet de lecture dit la boîte** derrière le nom, carte dépliée
   et rangée repliée.
5. **La vue d'un seul compte ne dit rien** (D7) : zéro bloc.

### 5.4 La gate

- `contraste.mjs` : même nombre de mesures (12 familles × 2 polarités ×
  5 fonds + glyphes), **libellé de la mesure à mettre à jour**.
- `coherence-systeme.mjs` : contrôle 6 (les listes de repères) inchangé ;
  contrôle 7 (icônes posées ⊂ catalogue) inchangé — **aucun glyphe neuf** ;
  contrôle 8 (zéro rayon) inchangé — le tracé n'introduit aucun littéral.
- `.repere.p16` **n'a plus d'emploi** : à retirer de `systeme.css` en même
  temps que sa mention au Système (§3.2, ligne 4). `.repere.p20` reste,
  pour les Réglages.

---

## 6. Les autres trouvailles de l'exploration

Elles sont **hors périmètre** de ce chantier. Elles sont ici pour ne pas
être perdues.

### 6.1 Le pied de la liste déborde à la borne basse — à confirmer au terrain

À 300 px de volet (la borne basse de `BORNES.liste`), les trois onglets
« Tous / Non lus / Brouillons » demandent **332 à 334 px** dans une
colonne qui en offre 299. Mesuré sur une maquette qui reprend la
grammaire du produit au pixel (`.onglets`, `gap:10px`, `padding:0 12px`,
onglets `height:32px; padding:0 14px`) — **ce n'est donc pas une preuve,
c'est une forte présomption**. Le pied ne dépend ni de la rangée ni de ce
chantier : **le produit livré déborderait déjà** à sa propre borne basse.

**Comment le confirmer** : ouvrir Wind, tirer la poignée liste à fond
vers la gauche, regarder le pied. Si le constat tient, c'est un terrain à
part entière — et les pistes seraient à explorer alors, pas maintenant.

### 6.2 Un nom de compte long n'est entier nulle part dans l'écran 02

La nav tronque elle aussi : sa rangée offre **172 px** au libellé pour
**199** nécessaires sur un nom de 32 caractères (`Nav.svelte`, `.libelle`
en `text-overflow:ellipsis`). Ajouté au tiers de la ligne (§1.3), il en
résulte qu'un nom long ne se lit **entier** qu'à l'infobulle et aux
Réglages. Si ça gêne, la question n'est pas la troncature : c'est celle
d'un **nom court** dédié (~12 caractères), demandé à l'ajout du compte.
Report, pas décision.

### 6.3 Le repère redevient facultatif — et c'est un gain

Aujourd'hui, deux comptes **sans repère** sont strictement indiscernables
en liste : le badge exige un repère. Avec la forme en lettres, le mot
suffit. C'est un argument de plus pour D8, et cela retire une pression
implicite (« il faut poser un repère pour s'y retrouver ») que le produit
n'avait jamais assumée.

### 6.4 Dans un fil, la mention se répète

Tous les messages d'un fil viennent de la **même boîte** : D5 fait donc
apparaître la même mention sur chaque carte. Elle reste juste, et elle
vaut pour le premier coup d'œil. Si la répétition gêne au terrain, la
porter **une seule fois** — sur la tête du fil, à côté du titre — donne
la même information pour un seul énoncé. Constat, pas contre-proposition.

### 6.5 Le nuancier tient en tracé, sans mesure neuve

Les 12 familles × 2 polarités, posées en **encre** sur `bg`, `hover`,
`sel` et `tuile` : pire cas **4,97:1**, pour un seuil composant de 3:1.
La marge est confortable, et elle explique pourquoi A82 ne coûte aucune
paire neuve à la gate.

### 6.6 Le piège du parseur (voir §4.2)

`lireThemes` ne capture pas les noms de jetons à trait d'union — c'est
ce qui laisse passer `--rep-*` sans gonfler la table des 17. **Protection
accidentelle** : à documenter en commentaire, sinon une « correction » de
la regex fera tomber le contrôle 1 sans que personne comprenne pourquoi.

### 6.7 Ce qui a été exploré et écarté — pour que rien ne se re-propose

22 dessins, cinq planches. Les raisons de chaque rejet sont écrites dans
les planches elles-mêmes ; les rejets structurants :

| | |
|---|---|
| Le liseré de compte (couleur seule) | **A8 rompu**, et le liseré est déjà le signal de la sélection : sur la rangée choisie, l'accent efface le compte. |
| Le sol teinté | A8, et le fond de rangée porte déjà quatre états ; une centaine de paires neuves pour la gate. |
| Les voies (une colonne par compte) | **133 px par voie** à 400 px, mesuré : l'aperçu ne tient plus. |
| Le décrochement (indentation par compte) | Un retrait n'a pas de nom : illisible sans légende. |
| Le pied à deux registres | Doublon avec la nav, et la grammaire des onglets **déborde de 11 px** à 400 px. |
| La bascule « grouper par boîte » | Deux flots à tenir : refuser de trancher se paie deux fois. |
| Le repli au seuil (V7) | Écarté par le CE au profit de la troncature (D4) : un libellé qui disparaît d'un coup surprend. |
| Le relais / le peloton (marque au changement) | **12 suites pour 14 rangées** sur un décor d'alternance réaliste : presque rien à grouper. **À re-mesurer sur de vraies boîtes** si l'idée revient. |

---

## 7. Terrain — la liste de contrôle du Chef Ingénieur

À dérouler sur de vrais comptes, après E5 :

1. Boîte unifiée : la ligne dit la boîte, et elle se **lit** — pas un
   refrain à la troisième rangée.
2. Vue d'un seul compte : le bloc a disparu, le texte ne s'est pas
   décalé (D7).
3. Un compte **sans repère** : la boîte se dit quand même.
4. **Poignée à fond à gauche** (300 px) : l'heure tient, le libellé
   s'ellipse, aucun débordement. *Regarder le pied au passage* (§6.1).
5. **Poignée à fond à droite** (640 px) : la ligne respire.
6. Dossier **Envoyés** : « À : X sur ▣ Boîte » se lit.
7. **Recherche** : le bloc est là (les comptes s'y mélangent toujours).
8. Volet de lecture **et** écran 03 : la mention est aux deux, derrière
   le nom.
9. Nav : le tracé au lieu de la pastille — le compte se trouve-t-il
   encore d'un coup d'œil ? *C'est le seul point où A82 peut coûter.*
10. Thème nuit : le tracé tient sur les quatre fonds.

**Ajoutés par la revue du 2026-08-25 :**

11. **Conversation épinglée** : le bloc prend l'encre chaude de la
    rangée, comme le reste de la ligne (R4) — aucun îlot gris froid.
12. **Volet de lecture dans la vue d'un seul compte** : la mention y
    est, alors que la liste se tait. C'est ce que dit D5, et c'est
    l'écart le plus discutable du chantier — *à trancher au terrain*.
13. **Dossier Brouillons** : la tuile aux initiales est toujours là
    (D9), et l'heure tient le bord droit (R2).
14. **Défilement profond** d'une grande boîte (Archives) : la rangée a
    changé de géométrie — la barre de défilement ne doit pas mentir, et
    les lignes ne doivent pas sauter. *C'est le point où le retrait de
    la tuile pourrait coûter, les hauteurs étant sondées.*
15. **Un poste à un seul compte** ne dit jamais la boîte (R3) — sans
    objet sur les quatre comptes du CE, à vérifier seulement si un
    poste de test n'en a qu'un.

---

## 8. Les planches de l'exploration

Jetables, hors production, à ne pas livrer — mais à garder tant que le
chantier n'est pas soldé :

| | |
|---|---|
| `spikes/volet-repere/planche.html` | Les 7 dessins de marque, et les trois arbitrages du retrait (D-a/D-b/D-c → D7/D8/D9). |
| `spikes/volet-repere/organisation.html` | Les 15 organisations du volet, 4 familles, avec leurs mesures. |
| `spikes/volet-repere/ligne-expediteur.html` | Les 7 formes de ligne + témoin, à 400 et 300 px, banc mesuré par la page. |
| `spikes/volet-repere/v1v7.html` | **La forme retenue en situation** : fenêtre entière, 3 largeurs, 2 polarités, nom long, volet de lecture, avant/après de la nav. |
| `spikes/volet-repere/o2.html` | La variante « glyphe seul » en situation — trace de ce qui a été comparé. |
| `socle.mjs`, `fenetre.mjs` | La matière commune : jetons et glyphes **lus du produit**, une seule implémentation de la rangée et de la fenêtre. |

---

## 9. Revue à regard neuf — 2026-08-25

Huit angles indépendants sur le diff (scan ligne à ligne, comportements
retirés, traçage inter-fichiers, **fenêtrage et sondes**, réutilisation,
accessibilité, tests, fidélité du Système), puis **un vérificateur
sceptique par candidat**, chargé de prouver le défaut depuis le code ou
de le réfuter. 40 candidats, 4 réfutés, **14 défauts distincts** après
regroupement. Tous corrigés avant la gate. Les quatre premiers étaient
invisibles aux gates vertes et à l'œil sur le décor d'essai.

| | Défaut | Remède |
|---|---|---|
| **R1** | `.boite` portait `min-width:0` alors que `sur` et le tracé sont `flex:none` : le bloc pouvait être **écrasé à 0 px** et son contenu **se peindre par-dessus l'heure** — 29,4 px de recouvrement mesurés au défaut de 400 px, dossier Envoyés. | `min-width:0` retiré du bloc : son minimum automatique garde le plancher « sur ▣ », `.lib` porte l'ellipse. **Pas** d'`overflow:hidden` en ceinture — sur un inline-flex il déplacerait la baseline, donc la hauteur sondée (A44). Garde e2e dans le dossier Envoyés à 300 px. |
| **R2** | La rangée du dossier **Brouillons** était la seule `.l1` sans `.essor` : `.exp` ayant perdu son `flex:1`, plus rien ne poussait son heure à droite. | L'essor posé comme partout ; garde e2e sur la distance au bord droit. |
| **R3** | **D7 testé sur la vue et non sur le nombre de comptes** : un poste à un seul compte recevait « sur \<sa propre adresse\> » sur chaque rangée — le refrain que D7 refuse. | La garde porte sur `comptes.length < 2`, en liste **et** au volet ; `comptes` descend de l'App au Fil. A80 amendé. Gardé des deux côtés par la spec de retrait de compte (bloc présent à deux comptes, absent après retrait). |
| **R4** | Le bloc échappait au remappage `--tuileInk` de la **rangée épinglée** : seul îlot gris froid sur le sol chaud, contre A73. | `.epingles .ligne .boite/.mot/.lib` prennent l'encre chaude ; le tracé garde la teinte du compte (paire déjà mesurée). |
| **R5** | L'infobulle disait « adresse — adresse » sur tout compte **sans nom personnalisé** — le cas par défaut de D8. | Un seul énoncé quand les deux chaînes sont identiques. |
| **R6** | Au **Système**, la règle `.boite` non qualifiée écrasait la géométrie de `.rang.boite` (la rangée de compte de la nav) : `max-width:33%` sur une rangée de 176 px. | Règle qualifiée par son contexte (`.l1 .boite, .carte .boite`). |
| **R7** | Dans la carte dépliée, le **plafond du tiers** se résolvait contre `.rang-nom` en shrink-to-fit : la règle écrite ne décrivait pas ce que le fil rendait. | `.tete-message .qui` prend `flex:1 1 auto`. |
| **R8** | La **gate de cohérence** ne contrôlait que la table de pastilles ; la table du **tracé** — qui porte désormais tous les emplois visibles — et les 24 jetons `--rep-*` y échappaient. | Contrôle 6 étendu aux deux tables et aux jetons des deux polarités. **Prouvé rouge** sur les trois pannes qu'il vise, vert sur le CSS réel. |
| **R9** | Un commentaire renvoyait la couverture du dossier Brouillons à une spec qui ne l'assertait pas. | Test neuf (tuile, classe `tuilee`, absence de bloc, heure à droite) ; le commentaire renvoie à un test qui existe. |
| **R10** | Deux règles produit sans aucun test : l'**exception de la recherche** et la forme **NOMMÉE** du libellé (branche `noms[…]`, jamais atteinte par les décors). | Un test ouvre une recherche depuis la vue d'un seul compte ; la spec du nom de compte vérifie « Boulot » et son infobulle. Le tracé au volet est assertionné dans la suite qui pose un repère. |
| **R11** | La règle du bloc vivait en **deux exemplaires** (Liste, Fil) et son titre y avait déjà divergé. | Extraite en fonction pure `lib/boite.js` ; le markup reste posé par chaque composant, comme la pastille. |

**Écarté, motivé** : l'absence de garde D7 *de vue* au volet de lecture
(deux vérificateurs sur trois l'ont réfutée) — D5 dit « le même schéma
au volet », le volet montre un fil déjà choisi et n'a pas de vue à
borner. À **regarder au terrain** tout de même (§7, point 12).

**Chiffres** : e2e 124 → **129** ; gate complète **verte en 1,8 min**
(9/9 étapes) ; contraste 220 paires, **aucune paire neuve** ; cohérence
du Système verte.

---

## 10. Terrain — verdict du Chef Ingénieur, 2026-08-25

**15/15.** Les quinze points du §7 passent. Quatorze OK à la première
passe ; **un seul constat, le point 12** — celui-là même que la revue
avait laissé ouvert plutôt que de le trancher seule.

> **Point 12 — « enlever la mention dans ce cas. »**
> Dans la vue d'un seul compte, la liste se taisait (D7) mais le volet
> de lecture disait encore la boîte. C'était la lettre de D5 (« le même
> schéma au volet de lecture »), et deux vérificateurs sur trois
> avaient refusé d'y voir un défaut. Le terrain a tranché :
> l'asymétrie se voit, et elle n'apprend rien.

**Corrigé le jour même, dans la même session.** La garde de vue est
sortie de `Liste.svelte` pour rejoindre la règle partagée :
`vueMelange(compte, enRecherche)` dans `lib/boite.js`, appelée par la
liste pour elle-même et par `App.svelte` pour le volet — l'App est la
seule à tenir les deux moitiés (le compte choisi, l'état de la
recherche). Une prop `melange` descend jusqu'à `Fil` par ses **deux**
cadres. **Deux expressions de la même règle auraient divergé** : c'est
le reproche que la revue avait déjà fait à ce chantier (§9, R11), il ne
se répète pas.

L'exception tient : une **recherche** lancée depuis la vue d'un seul
compte fait réapparaître la mention, liste et volet — les comptes s'y
mélangent toujours.

Amendements : A80 au Système (« le volet suit la MÊME règle que la
liste ») et la prose de l'écran 03. Garde e2e : le test de la vue d'un
seul compte ouvre désormais un fil et exige le silence du volet.
Re-gate **verte**, 129 e2e. **Point 12 revalidé OK par le CE.**

### Ce que le chantier laisse ouvert

Deux demandes neuves du Chef Ingénieur, **hors périmètre de ce
chantier**, instruites séparément (§2.6 — un chantier vert ne se retient
pas pour y greffer des features) :

| | |
|---|---|
| **Glyphes de repère en remplissage plein** | Touche la grammaire du jeu d'icônes (« trait de 2 unités ») et demande de savoir si les douze tracés sont des contours fermés qu'un remplissage rendrait, ou des traits ouverts qu'il déformerait. |
| **Trois niveaux d'espacement de la liste** | « Faible » (l'actuel), « Moyen », « Élevé ». L'espacement change la hauteur des rangées — or ces hauteurs sont **sondées**, et tout le fenêtrage en dépend (A44). C'est le point dur. |
