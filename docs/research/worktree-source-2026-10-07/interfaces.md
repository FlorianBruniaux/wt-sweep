# Interfaces et orchestration : Worktree Manager et Superset

[Retour au comparatif](../../worktree-source-analysis-2026-10-07.md).

Analyse du 07/10/2026 par l'intégrateur. Lecture ciblée du code et de deux
captures versionnées ; aucune application lancée, aucun test produit exécuté.
Les captures peuvent dater d'une interface antérieure à la tête clonée.

## Worktree Manager

Clone : `guoyongchang/worktree-manager`, commit
`c7eeb3e08e97c957ffa1d618a40c39dd86325971`, licence MIT ([LICENSE:1](https://github.com/guoyongchang/worktree-manager/blob/c7eeb3e08e97c957ffa1d618a40c39dd86325971/LICENSE#L1)).

La pile combine React, Tauri, Rust, git2 et des processus Git, avec PTY et un
serveur HTTP. Ce n'est pas une bibliothèque d'inventaire isolée. Un workspace
contient plusieurs projets, et une entrée worktree peut regrouper des checkouts
par projet. Cette organisation diffère de notre famille Git physique.

| Fonction observée | Mécanisme dans le code | Apport pour Sweep |
| --- | --- | --- |
| Inventaire du workspace choisi | [src-tauri/src/commands/worktree.rs:586](https://github.com/guoyongchang/worktree-manager/blob/c7eeb3e08e97c957ffa1d618a40c39dd86325971/src-tauri/src/commands/worktree.rs#L586) construit le chemin `worktrees_dir` configuré puis appelle `scan_worktrees_dir`. | Nommer clairement le périmètre connu. Ce n'est pas une découverte de toute la machine. |
| Collecte concurrente | [src-tauri/src/commands/worktree.rs:687](https://github.com/guoyongchang/worktree-manager/blob/c7eeb3e08e97c957ffa1d618a40c39dd86325971/src-tauri/src/commands/worktree.rs#L687) : threads scoped, file de travail sous mutex, 2 à 8 workers, cases de résultat conservant l'ordre d'entrée. L'entrée Tauri utilise `spawn_blocking` à la ligne 613. | Référence de séparation UI et calcul. Sweep possède déjà un nombre borné de workers ; aucune accélération supplémentaire démontrée. |
| Recherche et détails | [src/components/worktree-sidebar/ExpandedSidebar.tsx:660](https://github.com/guoyongchang/worktree-manager/blob/c7eeb3e08e97c957ffa1d618a40c39dd86325971/src/components/worktree-sidebar/ExpandedSidebar.tsx#L660) : recherche temporisée à 100 ms et surlignage, sans retirer les lignes. [src/components/WorktreeDetail.tsx:68](https://github.com/guoyongchang/worktree-manager/blob/c7eeb3e08e97c957ffa1d618a40c39dd86325971/src/components/WorktreeDetail.tsx#L68) : badges par fait ; `:538` : chemin copiable et infobulle complète. | Montrer chemin, branche, faits locaux, verdict et lacunes dans une même ligne détaillable. Différencier recherche qui surligne et filtre qui masque. |
| Archivage | [src-tauri/src/commands/worktree.rs:1328](https://github.com/guoyongchang/worktree-manager/blob/c7eeb3e08e97c957ffa1d618a40c39dd86325971/src-tauri/src/commands/worktree.rs#L1328) : verrou de cycle de vie, fermeture des PTY de l'application, inscription dans `archived_worktrees`. | Distinguer masquer dans une interface, sauvegarder, déplacer et supprimer. Ici les fichiers et branches sont conservés, mais les sessions PTY sont fermées. |
| Retrait définitif | [src-tauri/src/commands/worktree.rs:1977](https://github.com/guoyongchang/worktree-manager/blob/c7eeb3e08e97c957ffa1d618a40c39dd86325971/src-tauri/src/commands/worktree.rs#L1977) refuse une entrée non archivée ; `:2059` appelle `git worktree remove --force`, puis continue le nettoyage après les erreurs signalées. | Un état « archivé » ne prouve aucune conservation externe. Ne pas importer cette politique de retrait dans Sweep. |
| Interface d'agent | [packages/mcp/src/server.ts:75](https://github.com/guoyongchang/worktree-manager/blob/c7eeb3e08e97c957ffa1d618a40c39dd86325971/packages/mcp/src/server.ts#L75) assemble les outils `core`, `details` et `advanced` selon le niveau et le transport ; `tools/advanced.ts:153` expose archive et retrait. | Si un MCP devient nécessaire, publier d'abord des capacités de lecture. Le skill existant de Sweep suffit au besoin actuel. |

Le statut simplifié n'est pas un oracle de propreté. Dans
[src-tauri/src/git_ops.rs:217](https://github.com/guoyongchang/worktree-manager/blob/c7eeb3e08e97c957ffa1d618a40c39dd86325971/src-tauri/src/git_ops.rs#L217), les compteurs commencent à zéro et restent ainsi
si l'ouverture du dépôt ou `repo.statuses` échoue (`:229`, `:244`). Les ignorés
ne sont pas demandés par `StatusOptions` à cet endroit. Le composant de badges
rend « clean » en l'absence de badge (`WorktreeDetail.tsx:76`). Ce chemin de code
est une raison de conserver des états incomplets explicites dans Sweep ; il
n'a pas été reproduit en exécution ici.

Tests présents et lus : [src/components/worktree-sidebar/ExpandedSidebar.test.tsx:122](https://github.com/guoyongchang/worktree-manager/blob/c7eeb3e08e97c957ffa1d618a40c39dd86325971/src/components/worktree-sidebar/ExpandedSidebar.test.tsx#L122)
et `:142` vérifient la recherche sans masquage ;
[src-tauri/src/commands/worktree.rs:3804](https://github.com/guoyongchang/worktree-manager/blob/c7eeb3e08e97c957ffa1d618a40c39dd86325971/src-tauri/src/commands/worktree.rs#L3804) couvre le filtrage des archives,
`:3875` les préconditions du retrait, `:3978` un workspace sans sous-projets.
[src/components/WorktreeDetail.test.tsx:38](https://github.com/guoyongchang/worktree-manager/blob/c7eeb3e08e97c957ffa1d618a40c39dd86325971/src/components/WorktreeDetail.test.tsx#L38) utilise des composants simulés : ce
n'est pas un essai du comportement Git réel.

La capture `docs/screenshots/main-view.png` montre une navigation latérale,
des cartes par projet, des badges Git et des actions de terminal/éditeur. Pour
un rapport de 50 à 100 worktrees, retenir la navigation et les détails
progressifs, mais préférer un tableau aux grandes cartes répétées.

## Superset

Clone : `superset-sh/superset`, commit
`9a50076c324b3d2575762e5bdcba6838061c8be5`. Le fichier racine [LICENSE.md:1](https://github.com/superset-sh/superset/blob/9a50076c324b3d2575762e5bdcba6838061c8be5/LICENSE.md#L1)
déclare Elastic License 2.0. Les clauses à `:11` encadrent notamment le service
hébergé et les fonctionnalités sous clé. Une décision de reprise de code
nécessiterait un examen de licence ; cette analyse ne le réalise pas.

La pile desktop Electron/React s'appuie sur un service hôte TypeScript, tRPC et
SQLite via Drizzle. Les tables locales et les projections d'interface gèrent
projets, workspaces, sessions et PR. Reprendre cette infrastructure pour un
rapport HTML de Sweep serait disproportionné.

| Fonction observée | Mécanisme dans le code | Apport pour Sweep |
| --- | --- | --- |
| Métadonnées locales | [packages/host-service/src/db/schema.ts:231](https://github.com/superset-sh/superset/blob/9a50076c324b3d2575762e5bdcba6838061c8be5/packages/host-service/src/db/schema.ts#L231) sépare identifiant, projet, chemin, branche, HEAD, lien PR, type `local/worktree/session`, création, activité et archive. | Séparer provenance, fait Git et usage observé. Un workspace de type `local` partage le checkout ; ce n'est pas toujours un worktree physique distinct. |
| Filtres et modes | [apps/desktop/src/renderer/routes/_authenticated/_dashboard/v2-workspaces/stores/v2WorkspacesFilterStore/v2WorkspacesFilterStore.ts:121](https://github.com/superset-sh/superset/blob/9a50076c324b3d2575762e5bdcba6838061c8be5/apps/desktop/src/renderer/routes/_authenticated/_dashboard/v2-workspaces/stores/v2WorkspacesFilterStore/v2WorkspacesFilterStore.ts#L121) : recherche, projet, appareil, PR, agent, auteur, liste/board et tri persistés. | Adapter projet, chemin, verdict, lacunes et taille dans un HTML hors ligne. Ne pas ajouter des filtres sans données fiables derrière. |
| Priorités d'affichage | [apps/desktop/src/renderer/routes/_authenticated/_dashboard/v2-workspaces/utils/deriveBoardColumn/deriveBoardColumn.ts:70](https://github.com/superset-sh/superset/blob/9a50076c324b3d2575762e5bdcba6838061c8be5/apps/desktop/src/renderer/routes/_authenticated/_dashboard/v2-workspaces/utils/deriveBoardColumn/deriveBoardColumn.ts#L70) dérive six colonnes ; `.../components/V2WorkspacesList/V2WorkspacesList.tsx:51` affiche les besoins d'attention en premier. | Afficher les verdicts déjà émis par Sweep, sans les recalculer côté interface. Chez Superset une PR fusionnée passe avant un agent actif, règle de workflow qui ne prouve pas l'absence d'usage. |
| Vue compacte | `.../components/V2WorkspacesList/V2WorkspacesList.tsx:143` distingue hôte pas encore prêt, aucun résultat filtré et liste vide ; `:200` replie des groupes tout en gardant le groupe courant visible. | Très bon modèle pour séparer « rien trouvé », « rien visible » et « collecte incomplète ». Garder les totaux source et filtrés. |
| Retrait récupérable au niveau du suivi | [packages/host-service/src/trpc/router/workspace-cleanup/workspace-cleanup.ts:282](https://github.com/superset-sh/superset/blob/9a50076c324b3d2575762e5bdcba6838061c8be5/packages/host-service/src/trpc/router/workspace-cleanup/workspace-cleanup.ts#L282) écrit l'intention d'archive avant les opérations longues ; les échecs peuvent rendre la ligne visible à nouveau. | Reprendre l'idée d'état d'opération distinct du résultat final, pas la sémantique de suppression. Ce marqueur de base de données n'est pas une sauvegarde. |
| Réconciliation après interruption | [packages/host-service/src/runtime/archived-workspace-reconcile.ts:16](https://github.com/superset-sh/superset/blob/9a50076c324b3d2575762e5bdcba6838061c8be5/packages/host-service/src/runtime/archived-workspace-reconcile.ts#L16) sélectionne les retraits interrompus, revérifie l'absence de propriétaire puis reprend avec `force: true` à `:55`. | Référence pour tester les chemins réattribués. Sweep devrait revalider et demander une décision liée à l'état courant avant toute reprise destructive. |

Le chemin de suppression n'est pas notre contrat de conservation : le prévol
lit `hasChanges` et refuse les timeouts du pool (`workspace-cleanup.ts:300`),
mais le pipeline prévoit une suppression forcée. Il vérifie ensuite séparément
l'inscription Git et la présence des fichiers (`:579`), puis peut retirer un
reliquat dans une racine gérée (`:609`). Une copie de ce flux ne lèverait pas E1.

Tests présents : [packages/host-service/test/workspace-cleanup.test.ts:972](https://github.com/superset-sh/superset/blob/9a50076c324b3d2575762e5bdcba6838061c8be5/packages/host-service/test/workspace-cleanup.test.ts#L972)
couvre le timeout du prévol, `:503` un dossier survivant au retrait Git et
`:717` un reliquat hors racine gérée. Le test `:317` attend au contraire que
l'aperçu `inspect` retourne `canDelete: true` après certains échecs de lecture.
Il faut donc examiner la mutation réelle, pas convertir l'aperçu en preuve.
[packages/host-service/src/runtime/archived-workspace-reconcile.test.ts:19](https://github.com/superset-sh/superset/blob/9a50076c324b3d2575762e5bdcba6838061c8be5/packages/host-service/src/runtime/archived-workspace-reconcile.test.ts#L19)
protège un chemin possédé par une ligne vivante. Aucun de ces tests n'a été lancé.

La capture `apps/docs/public/images/workspaces-table.png` montre un tableau
repliable par projet avec recherche et filtres projet/appareil. La version
courante du composant lu groupe par statut. Pour Sweep, les deux modes peuvent
utiliser la même vue filtrée, avec regroupement initial par dépôt demandé par
Florian. Aucun rendu courant de Superset n'a été lancé ou évalué visuellement.

## Choix proposés

- Adapter les tableaux, filtres, détails repliables et états vides de Superset
  à un fichier HTML autonome alimenté par les artefacts Sweep.
- Reprendre les faits Git explicites et le chemin copiable de Worktree Manager,
  en conservant des lacunes visibles à côté des compteurs.
- Garder les fonctions de présentation pures et testables. La couleur, le
  groupe et l'ordre n'ajoutent aucune autorité au verdict.
- Écarter pour ce besoin l'application Electron/Tauri, le serveur HTTP, les
  PTY intégrés et les outils MCP destructifs. Ces composants répondent à une
  orchestration interactive, pas à la lecture d'un rapport.
