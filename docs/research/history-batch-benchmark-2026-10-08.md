# Lectures Git groupées et reprise, 08/10/2026

[Index](../README.md) · [Recette de mesure](../collection-performance.md#comparer-les-lectures-de-commits) · [Reprise](../discovery-resume.md)

## Résultat sur un corpus fixe

Les sessions groupées réduisent les processus Git de 146 à 39 pour l'historique
et de 180 à 45 pour l'historique privé. Chaque phase conserve 145 lectures,
33 054 octets d'objets et 145 étapes de graphe. Les huit observations de racines
privées restent couvertes par la même ancre nommée. Les OID, preuves, verdicts,
plafonds de phase et manifeste des sources sont identiques après normalisation
des dates, métriques et digests des conteneurs.

| Durée, en ms | Médiane avant | Médiane après | Étendue avant | Étendue après |
| --- | ---: | ---: | --- | --- |
| Commande complète | 21 032 | 12 146 | 20 872 à 23 704 | 11 411 à 14 124 |
| Historique | 5 597 | 2 104 | 5 449 à 5 877 | 1 369 à 2 754 |
| Historique privé | 7 641 | 2 384 | 7 413 à 8 954 | 2 167 à 2 513 |

La baisse observée de la durée totale médiane vaut 42,2 %. Elle concerne ce
corpus synthétique et cet hôte. Les autres applications ne sont pas contrôlées :
la charge moyenne à une minute se situe entre 74,7 et 109,3 sur 18 processeurs
logiques. Trois paires ne caractérisent pas une distribution générale de latence.

## Protocole et identité

Le runner crée une chaîne de 128 commits, une branche de 16 commits et une fusion
à deux parents. Trois worktrees utilisent des racines ORIG_HEAD et reflog éparses ;
le contrôle exige des lectures privées au-delà de ces racines déjà connues.
Un inventaire fixe sert aux deux binaires via `plan --inventory`. Un échauffement
précède trois paires alternées ; les huit échantillons sont valides et comparables. Les sources et
les exécutables sont empreintés avant et après chaque passage ; les atime sont
exclus du manifeste.

Environnement : macOS 26.5.1 arm64, Git 2.50.1 Apple Git-155, Python 3.12.14.
Le temps utilise une horloge monotone et exclut la création du corpus. Le cache
OS n'est pas vidé. La découverte est hors mesure : l’inventaire est déjà
enregistré. Les compteurs de processus par phase excluent les contrôles, alors que la durée de commande
inclut ceux-ci, la lecture des artefacts et la sortie du rapport.

- Baseline : `d99fefd67e6a533339b5100233aed0e9f53866da`, contenu fusionné par #22.
- SHA-256 baseline : `ff193a7bbf8a2d58c35eee68c434a43d976a8f2f506993f3bbcf469cc4fdfb32`.
- SHA-256 nouveau binaire : `a9bfd3e3f686a314712d9237ec9b4aa42e41e5e124b3ee76933b9f61047ca3fa`.
- SHA-256 du manifeste des 35 fichiers `src/**/*.rs`, `Cargo.toml`, `Cargo.lock` :
  `8e6feced3c33da4d5283a1c3a292d29b6f99c6e1021a74308705bcefc32fe31a`.
- SHA-256 du protocole : `b3b86a6a870edeeee047f2988115d0561bd97b285a7fcfea4fdea92d357c28a6`.
- SHA-256 du résultat brut : `94f8aea9c6439f82421902e678ca2e6d5df6839e0f484bbb5177bf5dad630774`.

Le manifeste de code trie les chemins avec `sorted()` sur des objets Python
`pathlib.Path`, puis écrit une ligne `chemin_relatif SHA256\n` par fichier.
Le nouveau binaire provient de la branche `codex/history-batches` avant commit.
Les résultats bruts et sorties JSON restent dans les artefacts locaux de la
campagne ; ce document conserve les mesures sans chemins de repos privés.

Le petit smoke test de 16 commits est comparable. La première campagne complète
a été interrompue par un calcul de médiane avant le premier échantillon mesuré ;
elle est exclue du tableau. La régression du runner passe, puis la campagne
ci-dessus repart dans une nouvelle sortie. Les sorties de la tentative restent
conservées ; son deuxième temps d'échauffement non persisté est inconnu.

Le plafond des objets et des phases est conservé. Le transport change : une
session dispose des octets restants de la phase, au plus 128 Mio, plus en-têtes
bornés et 64 Kio de stderr. Chaque commit reste limité à 8 Mio et chaque enfant
à 10 secondes. Le détail figure dans la référence CLI. Une trame mal formée,
un dépassement du transport ou une sortie Git en échec empêche la validation du
lot. Un refus de taille après la sonde préserve les objets indépendants déjà lus.

## Travail répété lors d'une reprise

Le test `repeated_bounded_passes_find_hidden_nested_repositories_without_duplicates`
compare 24 passages limités à deux répertoires et un scan neuf du même corpus.
Les deux retrouvent trois repos, dont un caché et un imbriqué, sans doublon.
Ils listent exactement 48 répertoires et 122 entrées. Un compteur indépendant
vérifie que chaque répertoire terminé n'est listé qu'une fois.

Les vérifications Git relisent en revanche les administrations connues :
372 lectures de préflight sur les 24 passages contre 18 pour le scan neuf.
Un échantillon isolé en mode debug mesure 36 174 ms cumulées contre 1 475 ms.
La reprise ne démontre donc pas un gain de temps face à un scan complet qui
peut finir en un passage. Elle permet de progresser sous un petit budget en
évitant les listings déjà terminés, avec un coût de revalidation à chaque fois.

Cette mesure exclut la création de fixture, le démarrage CLI et les écritures
de checkpoint sur disque ; elle inclut leur encodage/décodage en mémoire.
Ordre : reprise puis scan neuf ; cache OS non vidé, un seul échantillon,
aucune assertion de vitesse. Un répertoire interrompu doit être rejoué ; un
listing trop large peut exiger un budget d'entrées plus élevé. Les
répertoires terminés restent historiques et chaque intervalle reste explicite.
