# Feuille de route

[Index documentaire](README.md) · [Preuves du triage](finish-ledger.md#triage-t01-t09) · [Plan T01-T09](superpowers/plans/2026-10-05-worktree-triage-v2.md)

État du 06/10/2026 : T01 à T09 sont fusionnés dans `main` par la
[PR #1](https://github.com/FlorianBruniaux/wt-sweep/pull/1), commit
`ddda77ddba24566328a83dad79b74d943cecd8e5`. Le triage en lecture seule est livré.
L'objectif « quels worktrees puis-je nettoyer ? » reste partiellement satisfait
sur le gros dépôt essayé : 3 `keep`, 45 `verify`, aucun `cleanup_candidate`.
Les budgets de collecte locale et d'histoire laissent les 48 lignes avec des
inconnues. Aucun résumé local n'est complet sur HEAD, index et disque.

## Livré

| Périmètre | Comportement disponible |
| --- | --- |
| Découverte U1-U4, T08 | Un dépôt, plusieurs racines ou les volumes visibles ; origines, progression, interruptions et lacunes conservées. Le parcours global a été testé avec des volumes injectés. |
| T01-T04 | Cibles explicites, graphe Git, état local courant, stashes partagés et histoire privée, avec budgets et inconnues. |
| T05-T07 | Trois verdicts et conditions C1-C5, cibles par famille, filtres, groupes, contexte de commit, changelog et forge sans autorité sur l'intégration. |
| T09 | Commande `review`, aide par commande, inventaire original et rapports liés par un reçu. |
| Documentation | README anglais/français, tutoriel jetable, commandes pour un dépôt réel, prompt d'exemple, contrats et guides CLI. |

Les contrôles locaux et la CI réussie avant fusion sont détaillés dans le
[registre de livraison](finish-ledger.md#livraison-github-du-06102026).
Consulter le [workflow](https://github.com/FlorianBruniaux/wt-sweep/actions/workflows/ci.yml)
pour le statut d'un commit précis. La publication des sources ne constitue pas
une release binaire.

## Prochain travail : compléter l'analyse des gros dépôts

1. **Mesurer les lectures qui épuisent les budgets.** Sur une fixture
   représentative, relever séparément graphe, résumé local et vérification
   approfondie : durée, appels Git, objets/octets et worktrees entièrement
   analysés. Le diagnostic doit distinguer le coût des sous-processus du volume
   de fichiers et d'histoire.
2. **Corriger le coût démontré et rejouer les cas de tri.** Comparer avant/après
   sur la même fixture : conservation des verdicts à preuves égales, cas positifs
   de candidats, rétention du travail local et inconnues après refus ou limite.
   Relancer ensuite le dépôt réel explicitement choisi et publier la couverture
   obtenue. Zéro candidat peut être correct ; l'acceptation porte sur une analyse
   complète ou sur la cause précise de chaque preuve manquante.
3. **Valider une découverte multi-racines réelle.** Choisir les racines, comparer
   avec les familles connues, puis contrôler doublons, inscriptions extérieures
   et zones inaccessibles. Le scan global réel reste une exécution distincte,
   explicitement demandée.

Les extensions X01 (reprise), X02 (lectures Git groupées) et X03 (autre format
d'index) du [plan T01-T09](superpowers/plans/2026-10-05-worktree-triage-v2.md)
restent conditionnelles. Le déclencheur X02 sur la fixture de 100 worktrees n'a
pas été rencontré. La découverte réelle s'est achevée et son index v2 est pris
en charge : ni X01 ni X03 n'est justifiée par ce seul essai. Le diagnostic
ci-dessus doit établir le besoin avant de choisir une extension.

## Nettoyage automatique : chantier séparé

| Étape | Condition de sortie |
| --- | --- |
| E1 | Démontrer l'exclusion des écritures concurrentes, y compris depuis un descripteur déjà ouvert. Les expériences actuelles ne la démontrent pas. |
| U5 | Sauvegarder le contrat complet et restaurer indépendamment, sans accès au dépôt source, après validation d'E1. |
| U6 | Retirer et restaurer des fixtures jetables, avec fautes et interruptions injectées. |
| U7 | Revue indépendante et approbation humaine liée à la sélection exacte avant une exécution réelle. |

`apply` reste indisponible. Les conditions et les alternatives à comparer sont
consignées dans le [plan d'acceptation](plan.md) et la
[décision concernant le retrait](decisions/apply-readiness.md).
