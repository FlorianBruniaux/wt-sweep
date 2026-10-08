# Analyser ses worktrees avec un assistant

[Index documentaire](README.md) · [Tutoriel CLI](tutorial.fr.md) · [Configuration du dépôt](harness.md)

Le skill `wt-sweep` apprend à Claude Code et à Codex à utiliser la CLI, lire ses
rapports par étapes et proposer les vérifications utiles avant un nettoyage.
La CLI calcule les verdicts. L’assistant explique les résultats et les limites,
puis propose les prochaines actions de conservation ou de vérification.

Le skill est livré avec le dépôt. Il n’installe aucun service et ne demande
aucune clé d’API propre à wt-sweep. L’assistant utilise votre session habituelle ;
l’outil reste utilisable seul, sans modèle.

## Démarrer

Ouvrez une session Claude Code ou Codex **dans le checkout de wt-sweep contenant
le skill**. Le dépôt à analyser peut être ailleurs : son chemin est passé à la
CLI. Une session ouverte seulement dans cet autre dépôt ne dispose pas
nécessairement du skill.

| Client | Invocation | Fichier du dépôt |
| --- | --- | --- |
| Claude Code | `/wt-sweep` | [Skill Claude](../.claude/skills/wt-sweep/SKILL.md) |
| Codex | `$wt-sweep` | [Skill Codex](../.agents/skills/wt-sweep/SKILL.md) |

Ces emplacements suivent les conventions documentées par
[Claude Code](https://code.claude.com/docs/en/skills) et
[Codex](https://developers.openai.com/codex/skills/).
La découverte effective dépend du client, de sa version et de la confiance
accordée au projet. Après ajout du skill, ouvrez une nouvelle session et vérifiez
qu’il apparaît dans le sélecteur de skills. Les tests de fichiers ne prouvent pas
ce chargement.

Le skill utilise une CLI compilée sur macOS. Si elle manque, il peut compiler
les sources avec les dépendances déjà en cache. Un téléchargement manquant est
signalé. Il ne lance pas d’installation globale ou de scan de toute la machine
pour se préparer.

## Prompt pour un premier dépôt

Remplacez `/chemin/du/clone` par le chemin absolu du dépôt à examiner et confirmez
la référence d’intégration. L’exemple emploie `refs/heads/develop` ; gardez-la
seulement si c’est bien votre cible. Dans Claude Code, remplacez `$wt-sweep` par
`/wt-sweep`.

```text
$wt-sweep Analyse les worktrees du dépôt /chemin/du/clone.
La cible d’intégration confirmée est refs/heads/develop.
Produis un rapport HTML hors ligne et mesure l’espace disque.
Regroupe par répertoire. Donne d’abord le bilan global, puis examine les
worktrees par groupes de cinq. Explique les éléments à conserver, les preuves
manquantes et les candidats signalés par la CLI. Termine par une liste ordonnée
de vérifications et de suggestions pour préparer le nettoyage. Ne supprime rien.
```

L’assistant doit montrer le périmètre et les refs, vérifier l’aide de la CLI,
lancer une revue dans une nouvelle sortie et conserver son code de retour.
Il lit ensuite les artefacts sauvegardés, vérifie le reçu et donne le chemin
du HTML. Les messages de progression sont provisoires et ne remplacent pas le
rapport final. Les tailles allouées ne prédisent pas un gain récupérable sur APFS.

## Reprendre un rapport sans relancer l’analyse

Le chemin attendu est le sous-dossier `run-*` qui contient `run.json`,
`review.json` et `report.txt`, pas seulement leur dossier parent.

```text
$wt-sweep Lis le dossier de rapport /chemin/du/rapport/run-existant.
Ne relance pas de scan. Vérifie la cohérence des artefacts avec le reçu,
puis explique le bilan. Examine cinq worktrees à la fois et tiens la liste de
ceux déjà lus. Pour chaque verify, indique la preuve manquante et la prochaine
vérification utile. Distingue les observations de ce rapport de l’état actuel,
qui n’a pas été revérifié.
```

Un ancien rapport peut expliquer une décision passée. Il ne suffit pas pour
décider aujourd’hui d’une suppression. Un reçu cohérent ne prouve ni une
sauvegarde restaurable, ni la réussite de la dernière synchronisation sur disque.

## Ce que signifie « par étapes »

1. **Cadrer.** Le skill reprend les chemins et refs déjà donnés. Il demande
   uniquement ce qui manque pour choisir le bon périmètre ou la bonne cible.
2. **Établir le bilan.** Il présente les totaux source, la sélection affichée,
   la couverture et les limites de collecte.
3. **Lire les lignes.** Il examine les faits locaux et l’historique en petits
   groupes, conserve les verdicts et signale les lignes restant à lire.
4. **Proposer la suite.** Il distingue travail à préserver, preuve à compléter
   et candidat à réexaminer avant tout nettoyage.

Une page de lecture n’est pas une collecte supplémentaire. Un filtre d’affichage
n’accélère pas la collecte. Si un budget fixe a bloqué l’analyse, relancer la même
commande ne constitue pas une solution. Le skill doit expliquer la limite et
proposer une nouvelle exécution seulement si elle peut apporter une autre preuve.
Il commence avec `--budget standard`, inspecte les faits sauvés, puis propose un
passage ciblé `--worktree` avec `--budget thorough` si les plafonds standard sont
la cause des données manquantes. Il examine ce résultat avant de lancer toute la
famille avec le profil plus large. Les plafonds accrus ne garantissent pas que
la collecte termine et ne retirent aucun fichier ignoré du périmètre.
Pour mesurer le disque d'un seul dossier, le skill ajoute séparément
`--disk-usage --disk-root CHEMIN_ABSOLU`. `--worktree` limite les preuves Git et
locales ; il ne réduit pas la collecte disque. Le budget disque est indépendant
et utilise `--disk-budget standard|thorough`. Les autres racines restent listées
comme non demandées, et un total ciblé ne représente pas l'inventaire entier.
Les rapports de plusieurs exécutions conservent leurs dates et leurs périmètres.

Pour plusieurs racines, demandez leur inventaire puis confirmez les cibles par
famille. Le skill peut examiner chaque famille séparément ou employer la table
de cibles prévue par la CLI. Il ne devine pas une branche commune à tous les dépôts.

## Poursuivre une découverte interrompue

```text
$wt-sweep Lis le checkpoint de découverte et le dernier rapport indiqués.
Vérifie les options dans scan --help. Poursuis uniquement les racines enregistrées
avec --resume et un nouveau --checkpoint-out hors des dépôts inspectés.
Conserve chaque intervalle, explique la couverture et les lacunes du nouveau
passage. En cas de refus d'identité ou d'empreinte, ne modifie pas le checkpoint.
```

Ce parcours reprend la découverte de racines explicites. Les preuves de revue,
les fichiers locaux et le disque sont collectés à nouveau lorsqu'on les demande.
Le [guide de reprise](discovery-resume.md) décrit les cas sans progression et les
conditions qui imposent un scan neuf.

## Compléter une collecte bornée

```text
$wt-sweep Lis ce run et explique les limites de collecte par phase.
Distingue les worktrees keep déjà retenus des verify restant à résoudre.
Vérifie collection_metrics si ce champ existe, les plafonds appliqués et les
causes d’arrêt. Si le profil standard a atteint un plafond, propose une nouvelle
revue ciblée du chemin exact avec --budget thorough, dans une nouvelle sortie.
Inspecte ce résultat avant toute revue thorough de la famille complète.
Conserve les rapports précédents et leurs inconnues ; ne supprime rien.
```

`collection_metrics` sépare profil, plafonds appliqués, temps et compteurs de
chaque phase. Un ancien rapport peut ne pas contenir ce champ. Son absence ne
prouve ni une collecte rapide ni un travail nul. Le modèle et son niveau de
raisonnement restent un choix distinct du budget de la CLI.

## Modèle, raisonnement et sous-agents

Le skill choisit un profil selon le travail, puis vérifie ce que le client peut
appliquer. Les réglages suivants sont des points de départ à évaluer, pas un
classement mesuré des modèles.

| Travail | Codex, si disponible | Claude Code, si disponible |
| --- | --- | --- |
| Extraction de faits déjà collectés | GPT-6 Luna, `low` | Haiku, sans effort imposé s’il n’est pas pris en charge |
| Analyse courante et bilan | GPT-6.1 Sol, `medium` | Sonnet, `medium` si pris en charge |
| Interprétation contradictoire à examiner | GPT-6.1 Sol, `high` | Sonnet, `high` si pris en charge |
| Revue indépendante utile après investigation | GPT-6 Astra, `high` | Opus, `high` si pris en charge |

Pour un petit rapport, un seul agent suffit. Pour des lots indépendants, le skill
peut déléguer la lecture à deux sous-agents au maximum, dans la limite des places
réellement disponibles. Le coordinateur collecte une seule fois, répartit les
lignes ou familles, puis vérifie les retours contre les artefacts d’origine.
Les sous-agents ne relancent pas de scans et ne créent pas d’autres sous-agents.

Le skill applique les paramètres de modèle et de raisonnement exposés par l’outil
de délégation. Quand un contrôle manque, il conserve le réglage hérité et indique
la recommandation non appliquée. Il ne modifie aucun réglage persistant et ne
prétend pas changer le modèle de la session par une instruction textuelle.
Vos choix de modèle, limites de budget et restrictions gardent la priorité.

Un budget Git épuisé demande de nouvelles observations ou une correction de la
collecte. Un modèle plus puissant ne transforme pas ces données absentes en preuve.
Les [règles détaillées du skill](../harness/skills/wt-sweep/references/model-policy.md)
précisent les déclencheurs, les paramètres par client et le retour attendu.

Vous pouvez ajouter au prompt :

```text
Choisis le modèle et le niveau de raisonnement adaptés à chaque tâche.
Délègue uniquement les lectures indépendantes ou une contre-vérification utile,
avec deux sous-agents au maximum. Indique les réglages demandés, ceux réellement
confirmés par le client et les lots restant à lire.
```

## Bilan attendu

Le résumé commence par les nombres à conserver, candidats et à vérifier, puis
présente un tableau groupé par dépôt ou dossier :

| Élément | Information attendue |
| --- | --- |
| Identité | Chemin, branche et HEAD complet dans le détail |
| Décision | Verdict original `keep`, `cleanup_candidate` ou `verify` |
| Motif | Fait observé ou condition C1-C5 manquante |
| Travail local | Index, fichiers modifiés, non suivis ou ignorés, avec leur degré de collecte |
| Suite | Action de conservation ou vérification précise, sans suppression |

Les stashes partagés sont présentés au niveau de la famille. Les inconnues sont
expliquées : usage non établi, lecture incomplète, budget atteint ou autre motif
observé. L’assistant ne transforme pas une absence d’observation en absence de
travail.

`cleanup_candidate` reste un résultat de revue. `apply` est indisponible. Le skill
ne remplace pas cette commande par une suppression Git ou shell. La
[feuille de route](roadmap.md#nettoyage-automatique--chantier-séparé) décrit les
conditions encore nécessaires.

## Vérifier chez soi

Depuis le checkout contenant le skill, avec Python 3.11 ou ultérieur :

```sh
cargo build --locked --offline
python3 -B scripts/sync-ai.py --check
python3 -B -m unittest discover -s tests -p 'test_harness.py' -v
python3 -B -m unittest discover -s tests -p 'test_skill_workflow.py' -v
```

Le dernier test crée sa propre famille Git temporaire. Il vérifie les recettes
de collecte et de lecture par pages, un candidat propre, la conservation d’un
fichier non suivi et l’absence de changement dans les worktrees inspectés.
Il ne scanne pas vos dépôts.

Pour vérifier le chargement natif, ouvrez une nouvelle session dans ce checkout,
invoquez le skill et demandez : « Lis ton SKILL.md et explique les étapes sans
lancer d’analyse. » Vérifiez le chemin effectivement chargé. Lancez ensuite le
prompt sur un dépôt que vous choisissez ou reprenez un rapport déjà produit.
Cette étape confirme un autre niveau que les tests automatiques : l’invocation
réelle du skill dans votre client.

## Maintenir le skill

Modifiez [la source commune](../harness/skills/wt-sweep/SKILL.md), ses références
et ses [scénarios de routage](../harness/skills/wt-sweep/evals/scenarios.json), puis
régénérez avec `python3 -B scripts/sync-ai.py --write`. Les copies Claude et Codex
ne s’éditent pas séparément. Le [guide du harnais](harness.md) détaille les
contrôles et leurs limites. Aucun hook global n’est installé par ce dépôt.

## Une collecte ciblée après un budget atteint

Après lecture du rapport, l’assistant peut proposer `--worktree` avec le chemin
absolu exact d’une ligne à vérifier. Cette option réduit les lectures de fichiers,
d’histoire et de contexte ; `--path-prefix` seul ne fait que filtrer l’affichage.
Le [tutoriel de collecte ciblée](tutorial.fr.md#approfondir-un-worktree-après-une-analyse-partielle)
fournit le bloc de commandes. Vérifier la présence de l’option dans l’aide du
binaire utilisé. Garder les deux rapports et expliquer les contrôles qui restent
incomplets après la relance.
