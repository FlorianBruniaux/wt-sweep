# Compléter Worktree Sweep pour décider quoi nettoyer

[Index documentaire](../../README.md) · [Contrat initial](../../plan.md) · [Architecture](../../architecture.md)

Statut : proposition du 05/10/2026. Les fonctions décrites ci-dessous restent à
implémenter. Base inspectée : `a551ae33d749c6ee947911a10926bb6decb46d26`.

**Objectif :** trouver les worktrees accessibles sur le Mac, les regrouper par
repo ou répertoire et expliquer, pour chacun, ce qui permet ou empêche son
nettoyage. Le premier jalon est un rapport utile sur un repo de plusieurs
dizaines de worktrees, avant d'étendre la recette à la machine.

**Architecture :** conserver le CLI Rust, son exécuteur Git contrôlé, la
découverte existante et la séparation collecte/jugement/présentation. Ajouter
une analyse progressive, une décision motivée et des filtres de consultation.
Pas de service permanent, de modèle IA ni de nouvelle interface graphique
nécessaire pour ce périmètre.

**Spécification produit :** les sections « Résultat attendu » et « Règles de
décision » ci-dessous. Le [contrat initial](../../plan.md) reste applicable aux
preuves et à toute future suppression. Les modifications de schéma prévues au
lot 1 doivent rendre explicite toute différence de sens.

Pour l'exécution ultérieure, utiliser `superpowers:executing-plans` ; si une
délégation est autorisée, `superpowers:subagent-driven-development`. Ce document
n'active ni agents ni opérations sur les repos utilisateur.

## Point de départ

La découverte par repo, racines ou volumes existe déjà. Le dernier essai réel
a trouvé 46 worktrees, mais les 46 sont restés à trier. Il a produit 21 manifestes
locaux incomplets ; 25 worktrees n'ont pas été collectés après épuisement du
budget commun. Aucune observation d'intégration n'a été produite. Ce constat
décrit cet essai, pas l'état actuel des worktrees.

Deux causes sont visibles dans le code inspecté :

- `review.rs` parcourt les références locales sans donner la priorité aux cibles
  demandées. Chaque relation inspecte arbres et contenus avant les conclusions
  simples. Le lecteur partagé est limité à 2 048 objets, 32 MiB et 60 secondes.
- `local.rs` partage une limite d'entrées entre plusieurs collectes. De gros
  index et répertoires générés saturent cette limite. Les derniers worktrees
  subissent le budget déjà consommé par les premiers.

Augmenter uniquement ces plafonds ne règle ni l'ordre du travail, ni son partage,
ni la lisibilité des résultats. Le plan conserve les bornes, mais rend le travail
progressif, mesurable et reprenable.

## Résultat attendu

Trois entrées de parcours : analyser un repo et tous ses worktrees enregistrés,
chercher dans plusieurs racines, ou chercher sur les volumes montés accessibles.
Le mode repo retrouve aussi les worktrees situés hors du dossier principal.
Un filtre d'affichage par chemin ne modifie pas le périmètre déjà analysé.

La table récapitulative groupe les résultats par repo ou répertoire. Chaque
ligne donne le chemin, la branche ou le HEAD détaché, l'état local, l'intégration,
le verdict et sa raison. Le détail contient les informations suivantes :

| Dimension | Informations à rendre |
| --- | --- |
| Identité | Repo commun, chemin complet, principal/lié, branche ou HEAD détaché, OID complet, présence et verrou Git avec sa raison |
| Travail local | Fichiers modifiés, indexés, non suivis, ignorés, conflits, opération Git en cours ; compteurs exacts ou explicitement partiels |
| Travail sauvegardé dans Git | Stashes du repo, commits du HEAD non atteignables depuis les cibles, refs et reflogs propres au worktree à conserver |
| Intégration | Cible choisie et OID observé, identité/ascendance, éventuelle égalité de contenu, différences et limites de lecture |
| Contexte | Derniers commits, fichiers concernés, PR/MR associée, extraits pertinents du changelog avec leur source |
| Usage et couverture | Signaux d'usage observés, données non collectées, erreurs, limites atteintes et date de l'observation |
| Décision | Candidat au nettoyage, à conserver ou à vérifier ; raisons, conditions restantes et prochaine action |

Les stashes sont présentés une fois par repo, avec des renvois depuis ses
worktrees. Git stocke les stashes dans `refs/stash` et son reflog ; cette ref est
partagée entre les worktrees. Une mention de branche aide à chercher, sans
identifier à elle seule le dossier d'origine. Sources :
[git-stash](https://git-scm.com/docs/git-stash#_description) et
[git-worktree, refs partagées](https://git-scm.com/docs/git-worktree#_refs).

Exemple de rendu visé, entièrement fictif :

| Chemin | Branche | Travail local | Intégration | Verdict et raison |
| --- | --- | --- | --- | --- |
| `/projects/demo/.worktrees/fix-menu` | `fix/menu` | Aucun détecté, collecte complète | HEAD inclus dans la cible retenue | Candidat ; vérifier l'usage avant toute action |
| `/projects/demo/.worktrees/export` | `feat/export` | 2 modifiés, 1 non suivi | Non établie | À conserver ; travail local présent |
| `/projects/demo/.worktrees/refactor` | `refactor/api` | Index non interprété | MR fusionnée, HEAD différent | À vérifier ; comparer ce HEAD et compléter l'état local |

## Règles de décision

La décision est distincte des observations Git. Aucun score ne transforme une
accumulation d'indices en preuve. Une absence de résultat n'est jamais un zéro.

1. **À conserver** si une raison positive suffit : checkout principal, verrou,
   travail local à préserver, opération Git en cours, usage actif observé ou
   historique propre au worktree non préservé. Afficher aussi les éventuelles
   inconnues ; elles n'effacent pas la raison de conserver.
2. **Candidat au nettoyage** si l'historique concerné est couvert par une cible
   explicite, l'état local utile est entièrement connu et ne contient rien à
   préserver, aucune protection ni activité n'est observée et les identités sont
   cohérentes pendant la collecte. C'est un candidat à une revue finale, pas une
   autorisation de suppression. L'absence de signal d'usage ne prouve pas que
   tous les processus d’écriture sont arrêtés.
3. **À vérifier** dans les autres cas : données manquantes, dérive, cible ambiguë,
   squash seulement suggéré, HEAD détaché non couvert, stockage inaccessible,
   ou état non pris en charge. Donner la vérification précise qui manque.

Un stash partagé, lisible et indépendant de l'administration du worktree ne
bloque pas tous les worktrees du repo. Il reste visible et n'est jamais supprimé,
appliqué ou assimilé à une sauvegarde complète. Un reflog propre contenant du
travail non préservé impose au contraire une action de conservation.

L'ascendance prouve une relation d'historique, pas le maintien fonctionnel de chaque
modification après un revert. Une égalité d'arbres ne conserve pas à elle seule
l'historique d'un HEAD détaché. Patch-id, titre de PR et mention de changelog restent
des indices. La documentation de
[git-merge-base](https://git-scm.com/docs/git-merge-base#_operation_modes) décrit
le test d'ascendance et distingue résultat négatif et erreur.

## Contraintes communes

- Parcourir les dossiers cachés, ignorés et imbriqués. Aucune exclusion implicite
  de `node_modules`, `.worktrees`, `.claude` ou `.codex`. Une zone non visitée reste
  visible dans la couverture ; un volume absent ne devient pas un dossier vide.
- Garder les identités physiques, les octets des chemins et les OID complets.
  Échapper les valeurs affichées sans les réutiliser comme identités.
- Ne pas exécuter de configuration Git, matérialiser de fichiers cloud, récupérer
  implicitement des objets, ni modifier les index ou les repos inspectés.
- Choisir la cible par repo. La configuration explicite prime ; en son absence,
  proposer les branches observées sans choisir silencieusement `main`. Distinguer
  branche locale et ref distante connue localement, avec leur fraîcheur inconnue
  si aucun relevé distant n'est disponible. Aucun fetch automatique.
- Ne pas confondre découverte complète d'un périmètre accessible avec instantané
  de toute la machine. Dates, interruptions, erreurs et exclusions restent dans
  les résultats, y compris après filtrage.
- Tester avec des repos de test temporaires et des volumes injectés. Aucun scan global réel
  ni nettoyage réel n'est une étape de développement de ce plan.

## Lots d'implémentation

Chaque lot commence par un test de comportement en échec et se termine par une
vérification positive et les refus correspondants. L'intégrateur possède les
contrats partagés, le raccordement CLI et les métadonnées Cargo. Tout autre
propriétaire de fichiers sera nommé avant une délégation effective.

### 1. Fixer le contrat de décision et reproduire la saturation

Fichiers : `src/model.rs`, `docs/contracts.md`, `tests/model.rs`,
`tests/inventory_support.py`, nouveau `tests/test_scale.py`.

- [ ] Construire une fixture synthétique contenant 46 worktrees, plus de 128
  références et plus de 20 000 entrées suivies/générées. Mélanger cas intégrés,
  divergents, sales et inconnus ; placer un gros worktree en premier et un
  candidat simple en dernier. Ne copier aucune donnée du repo utilisateur.
- [ ] Enregistrer lectures d'objets, octets lus, sous-processus et causes d'arrêt.
  Le test doit échouer si les derniers candidats restent inconnus uniquement
  parce que le premier a consommé tout le budget.
- [ ] Définir les schémas proposés `wt-sweep.inventory/2` et `wt-sweep.review/2` :
  identités, couverture par dimension, cibles par repo, observations typées,
  décision à trois états, motifs, actions attendues et état d'avancement.
- [ ] Séparer preuve d'historique et vérification de contenu. Une observation v1
  ne change pas de sens ; un import v1 ne remplit pas des champs v2 manquants avec
  des valeurs favorables. Les anciennes preuves conservent leur périmètre.
- [ ] Tester sérialisation, refus des schémas inconnus, chemins arbitraires et
  conservation de toutes les raisons. Garder le test de saturation rouge
  jusqu'au lot 2, identifié comme test de régression du lot en cours.

Fin observable : contrat documenté et test qui reproduit le défaut, avec un
résultat attendu non vide pour chaque catégorie. Aucun « tout à vérifier » ne
peut satisfaire cette fixture.

### 2. Rendre l'analyse Git progressive et équitable

Fichiers : `src/review.rs`, `src/git.rs`, `src/main.rs`, `src/run.rs`,
`tests/review.rs`, `tests/test_review.py`, `tests/test_scale.py`.

- [ ] Traiter les cibles demandées avant les refs intermédiaires. Résoudre les
  cibles par famille, réutiliser les objets vérifiés et recontrôler les refs avant
  publication. L'analyse de toutes les autres branches devient un approfondissement.
- [ ] Vérifier d'abord les commits nécessaires à l'identité et à l'ascendance.
  Réserver la lecture complète des arbres/blobs à la vérification de contenu,
  déclarée séparément. Ne pas supprimer les contrôles d'intégrité : les preuves
  publiées doivent nommer exactement les objets et propriétés vérifiés.
- [ ] Faire progresser les worktrees par tours, avec budgets par travail et
  plafonds globaux. Un timeout local ne doit pas empêcher les cas simples suivants.
  Les résultats déjà établis restent utilisables même si une autre phase s'arrête.
- [ ] Réduire les sous-processus via lectures groupées bornées lorsque cela
  apporte un gain mesuré ; conserver neutralisation de configuration, contrôle
  des statuts, timeouts, limites de sortie et absence de téléchargement implicite.
- [ ] Publier la progression sur stderr et les résultats partiels dans un
  répertoire d'exécution propre. Préparer une reprise par unité terminée ; après
  reprise, revalider identités, HEAD et cibles. Ne jamais réutiliser comme frais
  un état local simplement parce que taille et mtime sont inchangées.
- [ ] Tester identité, ascendance, commits divergents, cible secondaire, objet
  corrompu/manquant, historique tronqué, refs remplacées et HEAD concurrent.
  Le test de saturation du lot 1 doit maintenant passer pour l'analyse Git.

Fin observable : chaque worktree reçoit son analyse Git ou une cause locale
précise ; une lecture coûteuse ne prive plus les autres du premier passage.

### 3. Donner un état local et des stashes exploitables

Fichiers : `src/local.rs`, `src/git.rs`, `src/review.rs`, `src/model.rs`,
`tests/local.rs`, `tests/test_enriched.py`, `tests/test_scale.py`.

- [ ] Séparer inventaire rapide de l'état Git et manifeste détaillé des contenus.
  Un dossier ignoré peut être signalé immédiatement sans prétendre avoir compté
  ou vérifié tous ses fichiers. Son approfondissement reste requis avant une
  conclusion qui dépend de son contenu ; aucun dossier n'est déclaré supprimable
  à partir de son nom.
- [ ] Comparer HEAD, index et fichiers pour distinguer staged/unstaged, suppressions,
  ajouts, conflits, non-suivis et ignorés. Ne pas déduire « propre » de stat seul.
  Traiter les index v4/split/sparse dans des tests dédiés avant de les reconnaître ;
  à défaut, conserver une cause explicite au lieu d'un message « malformé » générique.
- [ ] Lire les stashes une fois par repo : OID, date, message, base et fichiers
  concernés si lisibles. Afficher les rapprochements de branche comme indices,
  sans attribution certaine à un worktree. Ne pas exposer le contenu des secrets.
- [ ] Examiner les refs/reflogs propres au worktree et les opérations Git en
  cours. Distinguer HEAD détaché préservé et commits connus seulement par ces
  métadonnées. Une analyse bornée des refs reste partielle si la borne est atteinte.
- [ ] Détecter sous-modules, repos imbriqués, types spéciaux et fichiers non
  résidents ; ne pas les absorber dans un compteur « propre ». Ils gardent leur
  état ou un motif de vérification distinct.
- [ ] Tester un fichier différent dans HEAD/index/disque, un ignoré sensible,
  deux worktrees partageant des stashes, un reflog exclusif et une mutation à
  taille/mtime identiques. Vérifier indépendamment l'absence d'écriture.

Fin observable : la fixture de 46 worktrees produit un tri local utile, même si
l'inventaire détaillé de certains gros dossiers doit être repris. Des stashes
communs ne rendent pas artificiellement tous les worktrees inexploitables.

### 4. Compléter la découverte machine et la consultation

Fichiers : `src/discover.rs`, `src/mounts.rs`, `src/main.rs`,
`src/presentation.rs`, `src/output.rs`, `src/run.rs`, `tests/discover.rs`,
`tests/mounts.rs`, `tests/cli.rs`, `tests/test_inventory.py`.

- [ ] Éprouver les modes existants repo/racines/volumes avec worktrees cachés,
  temporaires, imbriqués et répartis sur plusieurs volumes. Inclure repos bare,
  administrations séparées, entrées absentes et liens cassés sans les réparer.
- [ ] Dédupliquer par identité physique et famille Git, pas par nom de dossier.
  Définir le traitement des alias/liens et des montages changeants ; signaler
  toute limite de parcours au lieu de faire disparaître le sous-arbre.
- [ ] Ajouter une reprise de découverte par racines/sous-arbres terminés. Un
  arrêt restitue résultats et périmètre restant ; la reprise ne revendique pas
  un instantané atomique. Stocker les checkpoints hors des repos inspectés.
- [ ] Ajouter regroupement par repo/répertoire et filtres par repo, préfixe de
  chemin, branche, état local et verdict. Afficher nombre total, nombre visible
  et couverture non filtrée. Éviter les faux préfixes comme `/app` et `/app-old`.
- [ ] Ajouter un parcours CLI unique pour enchaîner découverte, analyse et rapport,
  en réutilisant `scan` et `plan`. Les noms et options seront documentés avec leur
  première implémentation ; aucune commande future n'est présentée ici comme prête
  à lancer. Prévoir aide par commande, progression et reprise compréhensibles.
- [ ] Comparer l'ensemble trouvé à un registre de fixture indépendant, avec
  permissions refusées, interruption et ordre de visite différent. Tester les
  filtres sans modifier les verdicts ni perdre les résultats masqués.

Fin observable : tous les worktrees attendus dans le périmètre de fixture sont
retrouvés exactement une fois. Les zones non couvertes et le travail restant sont
consultables. Le filtre repo fonctionne quelle que soit la position des chemins.

### 5. Expliquer le travail et rendre le verdict actionnable

Fichiers : `src/review.rs`, `src/forge.rs`, `src/presentation.rs`,
`tests/review.rs`, `tests/forge.rs`, `tests/test_enriched.py`, `tests/cli.rs`.

- [ ] Appliquer les règles de décision définies plus haut sans effacer les faits
  positifs lors d'une collecte partielle. Chaque motif conduit à une action :
  examiner un diff, préserver des fichiers, vérifier une cible, lire un stash,
  compléter une zone ou confirmer l'arrêt de l'usage.
- [ ] Présenter les commits non atteignables depuis les cibles et un résumé borné
  des fichiers concernés. Expliquer les squash/cherry-picks suggérés, les reverts
  observés et l'égalité d'arbre sans les confondre avec l'ascendance.
- [ ] Réutiliser les imports forge existants pour les PR/MR, avec hôte, projet,
  HEAD et date de capture. Séparer « requête ouverte » et « processus utilisant
  le dossier » ; ne plus résumer les deux sous un compteur `Active` ambigu.
- [ ] Lire les changelogs reconnus ou indiqués explicitement : chemin, révision
  ou empreinte, passage et éventuel numéro de PR. Leur absence n'empêche pas une
  preuve Git. Leur présence ne change jamais seule une décision défavorable.
- [ ] Ajouter des signaux d'usage locaux lorsque accessibles. Une observation
  positive protège le worktree ; résultat vide, permissions refusées ou collecte
  non demandée restent distincts. Ni PID absent ni `lsof -d cwd` ne prouve l'inactivité.
- [ ] Tester MR fusionnée avec nouveaux commits locaux, titre trompeur, changelog
  absent ou contradictoire, export périmé et accès processus refusé. Ajouter des
  cas positifs de candidat et de conservation, pas uniquement des refus.

Fin observable : en lisant une ligne puis son détail, l'utilisateur comprend
quel travail existe, ce qui est déjà intégré et la prochaine action justifiée.

### 6. Valider le parcours complet et actualiser les guides

Fichiers : `tests/test_scale.py`, `tests/cli.rs`, `README.md`, `README.fr.md`,
`docs/tutorial.fr.md`, `docs/cli.md`, `docs/contracts.md`, `docs/architecture.md`,
`docs/finish-ledger.md`, `CHANGELOG.md`.

- [ ] Tester le parcours complet sur 46 puis 100 worktrees synthétiques, dont
  quelques-uns plus coûteux que les autres. Comparer classements, couverture,
  compteurs de travail et résultats après reprise à l'oracle indépendant.
- [ ] Vérifier l'invariance des décisions avec l'ordre de découverte et le nombre
  de workers, à couverture équivalente. Tester interruption, checkpoint altéré,
  cache périmé, changement de cible et modification concurrente.
- [ ] Exécuter les vérifications du repo depuis sa racine :

  ```sh
  cargo fmt --all -- --check
  cargo clippy --all-targets --locked --offline -- -D warnings
  cargo test --locked --offline
  python3 -B -m unittest discover -s tests -p 'test_*.py' -v
  python3 -B scripts/sync-ai.py --check
  ```

  En développement, passer par `rtk` ou `rtk proxy` selon les instructions du repo.

- [ ] Refaire ensuite un essai en lecture seule sur le repo choisi par
  l'utilisateur, avec la cible d'intégration réellement retenue. Recompter les
  worktrees au moment de l'essai : 46 est un ancien relevé, pas une constante.
  Conserver le rapport hors du repo, sans intégrer de données privées aux tests.
- [ ] Faire le test machine lorsque ce périmètre est explicitement lancé :
  racines/volumes affichés, rapprochement des familles découvertes avec Git,
  erreurs macOS conservées. L'exactitude globale est démontrée sur fixtures ;
  sur la machine, rapporter seulement la couverture effectivement observée.
- [ ] Donner dans le tutoriel une commande prête à copier pour chaque parcours,
  un exemple de rapport, la reprise après arrêt et un prompt de revue. Séparer
  résultat de tests, résultat du vrai scan et capacité de suppression.

Fin observable : le rapport réel est exploitable sans diagnostic du programme
par l'utilisateur. Aucun échec de collecte n'apparaît comme « 0 modification » ;
chaque verdict a des raisons vérifiables. Aucun nombre minimal de candidats n'est
imposé au vrai repo, dont tous les worktrees peuvent légitimement être à garder.

## Ordre de livraison et revue

Exécuter 1 → 2 → 3 pour corriger d'abord le parcours par repo. Livrer ensuite 4
et 5, puis la recette 6. Le contrat de décision du lot 1 s'applique dès le premier
rapport intermédiaire. Mettre à jour le changelog avec chaque comportement livré.

Revoir en priorité : faux candidat après squash ou dérive de HEAD, fichiers
ignorés omis, stash attribué au mauvais dossier, reflog exclusif perdu, cache
périmé, premier worktree qui épuise la revue, compteur nul après une erreur et
filtre qui cache une lacune. Une revue indépendante ne doit pas être attribuée à
l'auteur du lot. La répartition modèle/effort sera fixée lors de l'exécution,
sans lancement d'agents par ce document.

Conserver les ajouts en commits séparés par comportement. Pour retirer un lot,
revenir sur son commit après contrôle des changements intervenus depuis ; ne
pas restaurer en bloc un checkout partagé. Les schémas versionnés permettent de
refuser explicitement un rapport incompatible au lieu de le réinterpréter.

## Suppression : un jalon distinct

Les six lots répondent à « quels worktrees examiner pour nettoyer, et pourquoi ».
Ils ne lèvent pas le blocage actuel de `apply`. Le rapport montre les candidats
et les données à préserver, sans bouton ni script de suppression prêt à exécuter.

Le [contrat E1/U5/U6](../../decisions/apply-readiness.md) exige encore une exclusion
des processus d’écriture démontrée, une sauvegarde durable et une restauration indépendante
avant tout retrait. Le prochain travail sur `apply` doit commencer par une
preuve de cette frontière sur fixture, pas par une commande de retrait autour
d'un verdict. Branches et stashes ne sont jamais supprimés avec le worktree.

La présence d'un fichier ignoré reste une raison de conservation ou de revue.
Le [RETEX testé](../../retex-sessions.md) montre qu'un retrait Git sans force peut
effacer ces fichiers. Il montre aussi qu'un manifeste de hashes ou un patch
seul ne constitue pas une sauvegarde complète.
