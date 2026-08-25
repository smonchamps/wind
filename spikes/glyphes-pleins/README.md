# spikes/glyphes-pleins — sujet écarté, et pourquoi

> **Verdict du Chef Ingénieur, 2026-08-25 : « Le trait suffit. »**
> Le sujet est **clos**. Aucun code de production n'a été touché.
> Refus de périmètre au titre de STANDARD §2.6 — à ne pas re-proposer
> sans raison neuve.

## La demande

« Permettre de pouvoir mettre des glyphes dont le remplissage couleur
est plein (pas dans un disque, on parle du glyphe en lui-même) » — au
sujet des douze **repères de compte**, qui depuis A82 se dessinent en
tracé nu à la teinte du compte.

## Ce qui a été fait avant de trancher

Une **instruction sur pièces** (deux angles indépendants : grammaire du
jeu, contraste et gates), puis une **planche** rendue dans le moteur
réel — `planche.mjs`, qui produit `planche-clair.png` et
`planche-nuit.png` : les douze repères au trait contre plein + trait,
aux **trois tailles d'emploi** (16 px nav, 14 px ligne, 12 px pastille
des Réglages), sur les **deux polarités**, à leurs vraies teintes. Les
tracés sont lus de `lib/icones.js`, les teintes des jetons `--rep-*` de
`systeme.css` : rien n'est recopié.

**Aucun fichier de production n'a été modifié.** La planche a coûté une
demi-journée ; le chantier en aurait coûté un à cinq jours.

## Les faits établis — gardés pour ne pas les remesurer

1. **Rien n'interdisait le remplissage.** Aucune règle écrite ne
   l'exclut, et le jeu en contient déjà quatre cas — dont trois dans le
   jeu dédié aux repères (`music_note`, `pets`, `sports_esports`). Un
   repère plein n'aurait rien inauguré, il aurait généralisé.
2. **Neuf glyphes sur douze se remplissent sans redessin** (mesuré par
   aire de Gauss après aplatissement des courbes). Trois résistent :
   `shopping_bag` (l'anse s'auto-ferme en pâté de 26,1 u²),
   `account_balance` (cinq sous-chemins sur six à 0,0 u²) et
   `music_note` (les deux à 0,0 u²) — aucun réglage de rendu ne les
   remplit, il faudrait les redessiner.
3. **Le plein SEUL amaigrit** plusieurs glyphes (`music_note` ×0,44,
   `account_balance` ×0,28, `flight` ×0,72) : seul **plein + trait** ne
   cassait aucun des douze.
4. **Le coût que personne n'attendait** : le remplissage **rapproche les
   silhouettes** — recouvrement moyen 0,24 → 0,47, `home` et `star` de
   0,51 à 0,84. Or le travail d'un repère est de distinguer douze
   comptes d'un coup d'œil. Le plein rend de la présence et retire de la
   distinction.
5. **Les gates étaient aveugles au sujet.** Le contrôle qui vérifie que
   le Système dit les tracés ne lit que la clé `d`, pas les clés de
   remplissage ; et la gate de contraste serait restée verte du premier
   au dernier jour, le remplissage réutilisant les mêmes teintes. Aucune
   des deux n'aurait rien prouvé ici.

## Le motif du refus

A82 a retiré la pastille le 2026-08-24 **en écrivant sa perte** — « le
fond coloré donnait au repère une présence à distance ; un tracé de
2 unités à 16 px pèse ~1,3 px : la nav dit le compte plus doucement ».
Le terrain du 2026-08-25 a ensuite validé le point qui demandait
précisément si le compte se trouve encore d'un coup d'œil.

L'instruction a chiffré **comment** remplir, avec soin. Elle n'a jamais
établi qu'il **fallait** remplir. Mis devant la planche, le Chef
Ingénieur a tranché comme le Système l'a déjà fait deux fois sur ce
genre de question (V5, V14) : par constat, pas par calcul.

## Rejouer la planche

```
node spikes/glyphes-pleins/planche.mjs
```
