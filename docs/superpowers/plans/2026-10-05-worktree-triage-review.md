# Critique architecture et plan : tri des worktrees

[Plan examiné, v1](2026-10-05-worktree-triage.md) · [Plan consolidé, v2](2026-10-05-worktree-triage-v2.md) · [Index documentaire](../../README.md)

Relecture du 05/10/2026 avec `critique-arch` et `critique-plan`, sur le code
`a551ae33d749c6ee947911a10926bb6decb46d26`. Le plan v1 est relu par son auteur ;
ce document ne constitue pas une revue indépendante. Aucun test produit ni scan
réel n'a été exécuté pour cette critique. Les observations ci-dessous viennent
du plan, du code et des contrats existants.

**Verdict : conserver le CLI et ses frontières de lecture ; corriger le contrat
de décision et le découpage avant d'exécuter le plan entier.** La préparation
n'est pas exprimée en pourcentage : aucun modèle de mesure ne justifie un tel
chiffre. La v2 permet de commencer par un correctif isolé et vérifiable.

## Architecture examinée

Un seul binaire collecte les observations Git et locales, applique un jugement
déterministe puis publie un rapport. La v1 ajoute analyse progressive, caches,
reprise persistante, formats v2, filtres et aide au nettoyage.

Deux choix restent adaptés au besoin : l'exécuteur Git contrôlé et la séparation
des observations, du jugement et de leur présentation. Le stockage hors des
dépôts inspectés et l'absence de dépendance à une forge en ligne doivent rester.

## B1. Bloquant : « candidat » n'a pas encore de prédicat testable

La v1 demande que « l'état local utile » soit connu et qu'il n'y ait « rien à
préserver », sans préciser les domaines obligatoires ni le traitement des
inconnues non bloquantes. Elle introduit aussi une preuve d'histoire moins
coûteuse sans arrêter son rôle exact dans le verdict.

Le risque est concret : le [juge actuel](../../../src/review.rs), fonction
`judge`, classe `integration_observed` à partir d'une relation sur une cible.
Il ne tranche pas la conservation locale. `enrich` indique explicitement que les
octets des anciens manifestes ne sont pas revalidés. Reprendre ce classement
sous le nom « candidat » produirait un verdict plus fort que ses preuves.

**Correction dans la v2 :** table de décision avec domaines obligatoires,
priorité des raisons de conserver, inconnues bloquantes séparées du contexte,
et activité toujours conditionnelle. La relation rapide d'histoire reste une
information ; le candidat requiert une vérification approfondie distincte.
L'égalité d'arbre seule ne suffit pas. Une mention de changelog ne tranche rien.

## Points majeurs

| Réf. | Défaut du plan v1 et source | Effet possible | Correction v2 |
| --- | --- | --- | --- |
| M1 | Les lots suivent les couches techniques ; le lot 1 crée deux formats et laisse un test rouge, alors que le verdict est raccordé au lot 5. Voir lots 1, 5 et ordre de livraison de la v1. | Pas de première livraison utilisable ; plusieurs lots doivent être fusionnés avant d'être vérifiables ensemble. | Première unité : priorité des cibles demandées, test rouge puis vert dans le même lot. Ensuite, chaque unité produit une ligne ou une capacité de rapport visible. |
| M2 | Cache entre exécutions et reprise de sous-arbres sont introduits avant le premier parcours utile. La revalidation proposée porte surtout sur identités et HEAD. | Un fichier local peut changer sans changer HEAD ; un nouveau dépôt peut apparaître dans un sous-arbre déclaré terminé. La reprise peut présenter des données anciennes comme actuelles. | Cache initial limité à l'exécution. Relance ciblée pour refaire une collecte. Reprise persistante soumise à un déclencheur mesuré et à une sémantique d'observations datées. |
| M3 | La répartition des budgets ne supprime pas le plafond dur de 20 000 entrées dans `collect_with`. `deliver` et `read_artifact` imposent aussi 32 Mio par artefact. Voir [local.rs](../../../src/local.rs), [main.rs](../../../src/main.rs) et [output.rs](../../../src/output.rs). | Un gros index reste inconnu après optimisation ; les données collectées peuvent ne produire aucun rapport publiable. | Séparer résumé et détail, borner la taille avant sérialisation, tester le gros index isolément et le rapport à sa limite. Prévoir explicitement la réduction du détail et les lacunes. |
| M4 | « Choisir la cible par dépôt » ne définit pas la sélection. Le CLI transmet actuellement un unique `Vec` de cibles à toutes les familles. Voir `plan` dans [main.rs](../../../src/main.rs). | Une ref portant le même nom dans deux dépôts pourrait recevoir le même rôle sans choix par dépôt. `main` et `develop` ne sont pas interchangeables. | Associer chaque cible à l'identité de sa famille Git. Sans correspondance explicite, continuer l'inventaire mais laisser l'intégration indéterminée pour cette famille. |
| M5 | L'état Git rapide n'a pas de méthode de lecture retenue. Le [contrat des index](../../contracts.md#inventory-and-local-manifests) documente déjà un effet de bord d'une lecture split-index. | Un raccourci par une commande Git supposée inoffensive peut modifier les métadonnées ; réimplémenter tous les formats d'index élargit fortement le chantier. | Commencer avec les formats ordinaires déjà décodés. Tester tout nouveau chemin de lecture sur fixture avec oracle de conservation. Un format non pris en charge reste localement inconnu. |
| M6 | La charge est définie surtout par 46 puis 100 worktrees, avec plus de 20 000 fichiers mélangés au même scénario. | Une fixture lourde et lente peut masquer le goulot exact ; le nombre de worktrees ne borne ni profondeur d'histoire, ni références, ni octets. | Matrice distincte : nombreuses refs, gros index, gros contenu ignoré, histoire profonde, puis combinaison représentative. Mesurer lectures, octets et processus avant de choisir une optimisation. |

La limite de sortie n'est pas seulement une question de mémoire : `plan`
revalide aussi les familles avant publication. Une famille devenue illisible
peut refuser la publication globale. Le plan doit préserver la frontière de
sortie ; il ne peut pas la contourner pour afficher « rapport sauvegardé ».
Ce cas entre dans M3 et dans la validation T08.

## Modes d'échec à couvrir

| Scénario | Point manquant dans la v1 | Comportement attendu |
| --- | --- | --- |
| Répertoire contenant un ignoré, mais lecture détaillée interrompue | Confusion entre quantité inconnue et absence de travail | Conserver le fait positif « contenu local présent » ; laisser son compte exact inconnu. |
| Objet Git corrompu après une relation rapide d'histoire | Promotion possible du résultat rapide | Garder l'observation d'histoire et refuser le verdict candidat tant que sa vérification approfondie échoue. |
| Une famille change ou devient illisible | Publication couplée à toutes les familles | Ne pas retirer les contrôles de sortie ; signaler séparément analyse partielle et publication refusée, puis permettre une relance ciblée. |
| Disque de sortie plein, JSON trop grand ou sortie remplacée par un lien | Rapport partiel assimilé à un succès | Aucun reçu final de réussite ; fichiers sans reçu signalés incomplets. Aucun écrasement ni écriture dans les worktrees. |
| Chemin de changelog pointant hors du dépôt | Nouvelle source de texte sans frontière de lecture précisée | Lire seulement les chemins autorisés, sans suivre une redirection extérieure ; échapper le texte affiché. |
| Accès processus refusé | Champ d'activité présenté comme nul | Afficher « usage inconnu » ; ne jamais déclarer l'inactivité. |

Pour un CLI, l'équivalent du diagnostic d'incident est : progression sur stderr,
cause liée à une phase et un chemin, résumé partiel lisible, puis relance limitée
à la famille concernée. Un daemon, un circuit breaker réseau ou une base de
données ne résoudrait aucun des défauts observés ici.

## Alternatives et coûts

| Choix | Gain | Coût ou limite | Décision |
| --- | --- | --- | --- |
| Augmenter tous les plafonds | Correctif court | Le premier gros cas reste prioritaire ; coût et absence de progression persistent. | Écarté comme correction principale ; ajuster un plafond seulement après mesure. |
| Script regroupant des commandes Git usuelles | Petit prototype de rapport | Il faut rétablir les garanties de lecture, de chemins et d'erreurs déjà acquises ; split-index reste à vérifier. | Utile comme oracle sur fixture, pas comme remplacement sans preuve. |
| Revue Rust en deux niveaux, cache par exécution | Réutilise l'existant ; fournit tôt les faits peu coûteux | Il faut nommer clairement la portée de chaque niveau de preuve. | Retenu. |
| Checkpoints et cache persistant dès le départ | Évite certaines relectures | Invalidation, format de reprise, publication et vieillissement deviennent une seconde architecture. | Déféré sous condition explicite dans la v2. |

Le CLI reste limité à macOS. Git, les formats d'index et les API macOS sont les
dépendances à éprouver ; aucun fournisseur cloud n'entre dans le chemin critique.
Les rapports peuvent contenir chemins et sujets privés : aucun envoi automatique
n'est prévu. Les valeurs sensibles de fichiers ne doivent pas être reprises dans
un résumé sous prétexte d'expliquer une différence.

## Critique du plan et consolidation

Les scénarios adversariaux, le maintien des inconnues et la distinction avec
`apply` sont conservés. Les défauts portent sur l'ordre et les contrats manquants,
pas sur l'absence totale de tests ou de limites.

Le MVP doit livrer un dépôt ciblé, un rapport compact, l'état local, les stashes,
l'intégration et un verdict motivé. Les fonctions machine, filtres et changelog
demandées restent dans le plan complet. Les caches persistants, commandes groupées
et nouveaux formats d'index sont des extensions conditionnelles, pas des
prérequis silencieux au premier résultat.

Les trois corrections prioritaires sont intégrées au
[plan consolidé](2026-10-05-worktree-triage-v2.md) :

1. Arrêter le prédicat « candidat » et la portée de chaque inconnue avant son implémentation.
2. Livrer T01 avec un test rouge puis vert ; aucun commit de livraison ne conserve un test obligatoire rouge.
3. Utiliser des unités verticales et une matrice de charge ; n'ajouter une reprise persistante qu'après son déclencheur.

La question décisive est maintenant tranchée dans le plan : **une preuve rapide
d'histoire autorise-t-elle le nettoyage ? Non. Elle explique l'intégration ; la
décision exige les autres contrôles et reste conditionnelle à une revue finale.**

Les dépendances, la frontière de démarrage, les critères et le repli sont dans
la v2. Ils restent à éprouver pendant l'implémentation ; cette relecture ne les
présente pas comme des comportements déjà livrés.
