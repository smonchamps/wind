# spikes/espacement — les bancs de PLAN-ESPACEMENT

Jetables, hors production, gardés tant que le chantier n'est pas soldé.
Ils ont tranché deux choses que le raisonnement seul aurait mal
tranchées.

| | |
|---|---|
| `sondes.mjs` | **Le banc qui décide de l'architecture des sondes.** Quatre variantes × cinq hauteurs de cadre, dans msedge (le moteur réel de WebView2), sur la géométrie exacte de la rangée. Réponse : garder les sondes montées en permanence ne coûte **rien** — à condition que leur cage soit **positionnée**. |
| `crans.mjs` | Les trois crans capturés dans l'**application réelle** avec son décor, pour le STOP visuel d'E1. Écrit trois PNG dans ce répertoire. |

## Ce que `sondes.mjs` a établi, et pourquoi il fallait le mesurer

Les gabarits de hauteur de la liste sont sondés au rendu. Pour qu'un
cran d'espacement réglable ne fasse pas mentir le fenêtrage, les sondes
doivent se re-mesurer — donc rester montées. On leur reprochait
d'ajouter alors ~203 px de défilement fantôme.

La protection qui semble évidente — enfermer les sondes dans une cage
`height:0; overflow:hidden` — **ne protège rien** :

| variante | fantôme à 120 px de cadre | à 150 | à 203 | h1/h2 |
|---|---|---|---|---|
| sondes retirées (l'ancien) | 0 | 0 | 0 | plus mesurables |
| permanentes, `absolute` | **85 px** | 55 | 2 | 88/115 ✓ |
| cage `height:0;overflow:hidden` | **85 px** | 55 | 2 | 88/115 ✓ |
| cage **+ `position:relative`** | **0** | **0** | **0** | **88/115 ✓** |

La cage nue n'étant pas positionnée, elle n'est pas le bloc conteneur
des sondes en `position:absolute` : elles se calent sur le cadre et
échappent au clip. Un `position:relative` suffit, et sans lui la
protection est un placebo — d'où le commentaire appuyé dans le CSS de
`Liste.svelte`, pour que personne ne « simplifie » cette ligne.

## Rejouer

```
node spikes/espacement/sondes.mjs
node spikes/espacement/crans.mjs
```

Playwright vit sous `e2e/` : les deux bancs vont l'y chercher
eux-mêmes. `crans.mjs` lance l'application (décor Clarity) et prend ses
captures ; `sondes.mjs` n'a besoin que d'un navigateur.
