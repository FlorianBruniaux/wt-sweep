# Critique du plan de comparaison locale

Statut : revue du plan, pas une revue indépendante du code ni une recette produit.
Lecture du 13 octobre 2026. [Plan examiné](2026-10-13-local-content-comparison.md),
préimage SHA-256 `a5a005a56472d32c60086ab81c742a18f555fefa0ea46f53c5dbe7e12df6a23d`.
La [v2 corrigée](2026-10-13-local-content-comparison-v2.md) remplace son découpage.

## Readiness score

**40 % : préparation insuffisante pour lancer l'implémentation.** Ce score est
un jugement qualitatif sur le plan, pas une mesure de code, de risque ou de délai.
Le périmètre est défini, mais le contrat de preuve et la couverture ne permettent
pas encore de vérifier les conclusions annoncées.

## What's solid

- Références explicites, comparaison au même chemin et aucun effacement fondé
  sur l'âge, le nom d'un cache ou le statut d'une PR.
- Index et disque séparés, inconnues conservées, artefact lié à la revue ;
  `review/2`, `triage/1` et C1-C5 restent inchangés.
- Fixtures Git réelles, positifs et contre-exemples, oracle avant/après pour
  contrôler les mutations et conservation des changements préexistants.

## Blockers

### B1. Les entrées nécessaires pour vérifier une égalité sont perdues

Le plan définit un `ReferenceFingerprint` temporaire, puis ne conserve dans
`ComparisonRow` que la source et des identifiants de références correspondantes
ou inconnues : [v1](2026-10-13-local-content-comparison.md), lignes 125–140.
La vérification prévue, lignes 278–286, peut contrôler des compteurs, mais ne
peut pas recalculer cette égalité avec des empreintes de référence absentes.
Un digest valide n'ajoute pas les observations manquantes.

**Correction :** conserver pour chaque référence le résultat de lecture,
contenu/type/mode ou absence observée, provenance, base de comparaison, OID ou
témoin filesystem et lacune. Recalculer résultat et compteurs depuis ces entrées.
Le vérificateur prouve leur cohérence et leur liaison au collecteur ; sans
relire les sources, il ne certifie pas l'authenticité d'un artefact entièrement
refabriqué ni l'état actuel du disque.

**Gate :** sur un positif à référence unique, altérer l'empreinte de référence
sans corriger le classement, ou altérer le classement seul, puis recalculer le
digest doit laisser une incohérence détectée par le validateur.

### B2. « Complet » n'est pas suffisamment lié à la collecte source

`WorktreeComparisonDraft` porte un booléen `complete`, des lignes, des omissions
et des lacunes, mais pas une comptabilité par catégorie liée au résumé source :
[v1](2026-10-13-local-content-comparison.md), lignes 164–175.
Le modèle de couches seul ne distingue pas non suivi et ignoré. Une liste vide
avec des compteurs internes nuls ne suffit donc pas à justifier qu'aucun
changement local n'a été omis.

**Correction :** lier le digest exact du `LocalSummary`, conserver la catégorie
source, les couches et un registre de couverture. Réconcilier les changements
observés avec les lignes émises, omissions et observations inconnues. Les
échantillons de `LocalCount` ne sont jamais une liste exhaustive. Absence de
collecte, collecte incomplète et collecte complète sans changement sont trois
états distincts.

**Gate :** une comparaison vide déclarée complète doit être refusée face à un
résumé source positif ou incomplet. Tester aussi un worktree sans changement
entièrement observé, qui doit pouvoir être complet.

## Major concerns

### M1. Le budget partagé peut dégrader la revue de base

Le plan prévoit de partager le budget du collecteur, sans définir l'ordre ni
l'isolation des arrêts : [v1](2026-10-13-local-content-comparison.md), lignes
248–264. Aujourd'hui, `charge` marque le collecteur arrêté lorsque le plafond
est dépassé ; cet arrêt invalide ensuite la complétude du résumé local :
[`summary.rs`](../../../src/local/summary.rs), lignes 118–137 et 993–1008.
Un coût supplémentaire de comparaison pourrait donc produire de nouveaux
`verify` au lieu d'un simple bilan partiel.

**Correction :** terminer les phases de revue de base d'abord, conserver les
empreintes source sous enveloppe bornée, puis comparer avec l'allocation
restante. Un arrêt propre à la comparaison ne modifie pas rétroactivement le
résumé source. Ne pas doubler les plafonds ; conserver les contrôles de dérive
et les intervalles. Une source qui a réellement changé ne devient pas stable.

### M2. Le nombre de lignes ne borne pas la mémoire ni la publication

Le plafond de détails ne remplace pas un plafond global en octets. Multiplié
par les worktrees et les références, il peut produire un document trop gros,
avec un refus seulement à la publication : [v1](2026-10-13-local-content-comparison.md),
lignes 300–301. Le publisher dispose déjà de plafonds et de
`bounded_json_size` : [`output.rs`](../../../src/output.rs), lignes 607–619 et
699–718. Ce contrôle final ne borne pas les vecteurs déjà construits.

**Correction :** réserver progressivement une enveloppe globale pour les
lignes, témoins, références et lacunes ; inclure les métadonnées minimales.
Une omission donne une couverture partielle explicite, jamais un faux zéro.
Si même les métadonnées obligatoires ne tiennent pas, refuser la publication.
Faire dépendre le statut du reçu de la revue **et** de la comparaison demandée.
Aujourd'hui `RunInfo.partial` ne couvre pas ce nouvel artefact :
[`main.rs`](../../../src/main.rs), lignes 896–904 ;
[`run.rs`](../../../src/run.rs), lignes 235–241.

### M3. Le découpage retarde la première fonction utilisable

LC1 à LC4 construisent les types, la projection, les lecteurs et la publication.
Le raccordement CLI arrive en LC5, les rendus en LC6. Le risque principal n'est
pas testé assez tôt : peut-on donner une réponse compréhensible, reliée aux
preuves, sans modifier le verdict ni perdre la couverture ?

**Correction :** après un checkpoint de préservation, livrer une comparaison
Git minimale de bout en bout, avec CLI, preuve liée, reçu, texte et HTML. Ajouter
ensuite le checkout explicite et les cas multi-références. Les frontières,
plafonds, inconnues et contrôles de publication font partie du premier lot.

### M4. Le retour arrière n'est pas exécutable sur un checkout sale

LC0 conserve des préimages ; LC8 prévoit des contrôles. Le plan n'explicite pas
comment annuler uniquement les changements d'un lot sans restaurer `HEAD` et
perdre les modifications déjà présentes ou concurrentes.

**Correction :** checkpoint par lot avec préimages, postimages et fichiers
créés possédés. Un outil temporaire vérifie toutes les postimages avant de
restaurer les seules préimages du lot. Au premier changement concurrent, il
refuse la restauration automatique. Tester ce mécanisme sur une fixture sale
avant de l'utiliser. Aucun `reset` global ni suppression de répertoire partagé.

## Missing elements

- **Modes de l'index :** une entrée Git ne porte pas un mode POSIX complet.
  Une comparaison contre un checkout ne doit pas inventer ce mode pour annoncer
  `matches_reference`. La base de comparaison doit être explicite ; ce cas
  reste inconnu tant que la preuve demandée manque.
- **Lecture du bilan :** « aucun contenu différent observé », « tous les
  changements comparés ont un équivalent » et « worktree supprimable » ne sont
  pas interchangeables. Un doublon ignoré reste `keep` sous le contrat actuel.
- **Statuts :** distinguer feature désactivée, worktree volontairement non
  collecté, comparaison partielle et erreur fatale de publication. Un filtre
  ne réduit ni les références ni la couverture source.
- **Évidence de revue finale :** prévoir un reviewer distinct de l'auteur après
  autorisation de la méthode d'exécution. Cette critique n'est pas cette revue.

## Delivery graph

Les unités corrigées et leurs gates sont détaillées dans la v2.

| Unit | Delivers | Blocked by | Validation |
| --- | --- | --- | --- |
| U01 | Préservation et contrat de preuve vérifiable | Aucune dépendance technique | Rollback testé sur fixture sale ; règles de couverture explicites |
| U02 | Comparaison contre une cible Git, CLI → artefact → HTML/texte | U01 | Égalité positive, octet différent, index/disque séparés, corruption refusée |
| U03 | Copie locale explicite dans la même famille | U02 | Ignoré égal/unique, modes, frontières, dérive et absence |
| U04 | Plusieurs cibles et enveloppe de run à grande échelle | U03 | Égalité avec lacune, limites injectées et reçu partiel cohérent |
| U05 | Parcours assistant et documentation | U04 | Projections canoniques et consignes fidèles aux résultats |
| U06 | Recette intégrée et revue indépendante | U05 | 14 cas, suites obligatoires et oracle de conservation |

**Frontière technique : U01.** Aucun lot produit n'a démarré. Le choix de méthode
et la validation humaine de la v2 précèdent l'exécution. Les unités partagent
le collecteur, le publisher et les rendus ; elles sont séquentielles.

## Essential / defer

**Essentiel :** toutes les fonctions et les 14 cas de la spécification approuvée,
liaison des preuves, couverture, budgets, absence de mutations, affichage hors
ligne et préservation du checkout. Aucun de ces gates n'est repoussé après la
livraison finale.

**Différé :** recherche sous un autre chemin, normalisation de configurations,
heuristiques « cache jetable », nouvelle politique de retrait, suppression,
réseau, dépendance ou refonte de recherche HTML. Ces extensions nécessitent un
besoin démontré et une nouvelle décision de périmètre.

## Top 3 actions

1. Figer les entrées de preuve persistées et la comptabilité de couverture.
2. Fermer U02 de bout en bout avant d'étendre les lecteurs et les références.
3. Tester les plafonds, le reçu partiel et le rollback sur fixtures possédées.

## Limites de cette critique

Les constats viennent du plan et des portions de code référencées. Les scénarios
de faux bilan sont des contre-exemples à tester, pas des bugs exécutés dans une
fonction existante : cette fonction n'est pas implémentée. Aucune suite produit,
suppression, publication ou modification de configuration globale n'a été faite
pour cette revue.
