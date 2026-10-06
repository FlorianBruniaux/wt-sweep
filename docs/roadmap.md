# Feuille de route

[Index documentaire](README.md) · [Preuves du triage](finish-ledger.md#triage-t01-t09) · [Plan T01-T09](superpowers/plans/2026-10-05-worktree-triage-v2.md)

État du 06/10/2026 : T01 à T09 sont fusionnés dans `main` par la
[PR #1](https://github.com/FlorianBruniaux/wt-sweep/pull/1), commit
`ddda77ddba24566328a83dad79b74d943cecd8e5`. Le triage en lecture seule est livré.
Le checkpoint initial portait sur 48 worktrees. Le dernier essai du lot de
collecte ci-dessous porte sur 49 : 4 `keep`, 45 `verify`, aucun candidat. Les
49 résumés locaux restent partiels. Un passage ciblé conserve une ascendance
vérifiée et observe au moins 4 969 fichiers ignorés ; ce worktree devient `keep`.
L'objectif « quels worktrees puis-je nettoyer ? » reste donc partiellement
satisfait sur ce gros dépôt, même après réduction du coût des lectures.

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

## Accompagnement par un assistant

Le [skill du dépôt](assistant-skill.md) fournit un parcours de revue et de lecture
progressive pour Claude et Codex, avec des suggestions de conservation et de
vérification. Il ne résout pas les budgets de collecte et ne débloque pas `apply`.
Les preuves de génération, de routage et d’utilisation de la CLI sont distinguées
dans le [registre courant](progress.md#skill-wt-sweep-06102026).

## Prochain travail : compléter l'analyse des gros dépôts

Le lot de collecte du 06/10 groupe les lectures d’arbres et de blobs, partage le
parcours de chaque cible Git et répartit le budget local entre les worktrees.
`--worktree` permet un nouveau passage ciblé, avec conservation de l’inventaire
original. Les régressions et les limites restantes sont suivies dans le
[registre de collecte](collection-performance.md). Les étapes ci-dessous restent
les critères de mesure et de validation ; les plafonds ne sont pas des preuves
de complétude.

1. **Reproduire les plafonds restants.** Ajouter une fixture dépassant 4 096
   objets distincts et une histoire privée comparable au cas réel. Mesurer le
   temps et les lectures par phase, y compris les revalidations. Les fixtures
   actuelles à 1 024 contenus prouvent le gain des lots, pas la couverture du dépôt réel.
2. **Choisir une collecte qui termine ce cas.** Comparer partage des objets déjà
   vérifiés, reprise bornée et limites explicitement configurables. Conserver
   revalidation finale, mémoire bornée et refus après erreur ; toute extension
   doit produire un cas positif complet et préserver les cas de travail local.
   Rejouer ensuite le dépôt explicitement choisi, avec des causes précises pour
   les preuves encore manquantes. Augmenter le délai seul ne lève pas le plafond d’objets.
3. **Valider une découverte multi-racines réelle.** Choisir les racines, comparer
   avec les familles connues, puis contrôler doublons, inscriptions extérieures
   et zones inaccessibles. Le scan global réel reste une exécution distincte,
   explicitement demandée.

X02 (lectures Git groupées) est désormais utilisé par le résumé local : les
fixtures à 1 024 contenus distincts ou dossiers distincts reproduisent le coût
que la fixture initiale à contenu partagé ne montrait pas. Les extensions X01
(reprise) et X03 (autre format d’index) du [plan T01-T09](superpowers/plans/2026-10-05-worktree-triage-v2.md)
restent conditionnelles. La découverte réelle s’est achevée et l’index v2 est
pris en charge. Une collecte ciblée est un nouveau run, pas une reprise X01.
Le support d’un autre format d’index demande un cas concret et ses fixtures.

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
