# Comparaison locale : plan d'implémentation v2

Statut : plan approuvé dans la conversation du 13 octobre 2026, implémenté dans
le checkout local ; la [recette](../../research/local-comparison-acceptance-2026-10-13.md)
consigne les contrôles et leurs limites. La
[spécification approuvée](../specs/2026-10-13-local-content-comparison-design.md)
garde son périmètre. Le [plan v1](2026-10-13-local-content-comparison.md) reste
disponible comme archive. Les unités ci-dessous conservent le découpage accepté ;
la recette donne les interfaces et tests effectivement livrés.

## Résultat et frontières

Afficher « Que reste-t-il à récupérer ? » pour chaque worktree : chemins présents
dans une cible Git nommée, présents dans la copie locale explicite, différents
ou non comparés. Conserver séparément index et disque, preuves et couverture.
Le bilan aide à examiner les changements ; il ne décide pas de leur utilité.

- Garder `review/2`, `triage/1`, C1-C5 et les verdicts existants. Un doublon
  ignoré peut expliquer un `keep`, sans le transformer en candidat de retrait.
- `apply` reste indisponible. Aucun fetch, prune, gc, retrait ni suppression de
  branche réelle. Aucun scan global automatique, secret publié ou nouveau réseau.
- `review` et `plan` proposent `--compare-local` ;
  `--compare-reference ABSOLUTE_PATH` exige cette option et la même famille.
  Sans référence explicite applicable, refuser la demande. Ne rien deviner.
- Garder les chemins `BytePath`, OID complets, mode Git/POSIX annoncé,
  provenance et intervalles. Ne pas normaliser le contenu ou suivre les liens.
- Standard/thorough restent les profils bornés existants. Le plafond proposé
  de détails est 4 096/32 768 ; une enveloppe globale en octets peut arrêter
  plus tôt. Aucune hausse des allowances pour obtenir un bilan complet.
- Bibliothèque standard et dépendances déjà présentes. Aucun changement de
  Cargo attendu. L'intégrateur possède contrats, raccordement CLI et publication.
- Préserver le checkout sale et les changements disque/HTML préexistants.
  Avant les edits, lire les documents imposés par `AGENTS.md`, notamment les
  contrats et les règles de cibles/filtres. Utiliser `rtk proxy` pour les sorties
  Git exactes. Les commandes Git du produit gardent leur transport contrôlé.

## Contrat à fermer avant la première livraison

### Preuves persistées et jugement pur

`local-comparison.json`, schéma `wt-sweep.local-comparison/1`, porte les digests
canoniques de l'inventaire et de la **revue source**, son propre digest, les
budgets et les intervalles. Chaque worktree lie aussi le digest exact du
`LocalSummary` utilisé. Un digest de vue filtrée ne remplace pas la revue source.

Une observation conserve chemin relatif, catégorie source (staged, unstaged,
conflict, untracked, ignored), couche et éventuel stage. Une suppression garde
sa catégorie et sa couche d'origine ; elle n'est pas un fichier de zéro octet.
Pour chaque référence demandée, conserver :

| Donnée | Sens et contrôle |
| --- | --- |
| Identité et provenance | Ref Git capturée ou checkout physique explicitement choisi |
| Base de comparaison | Contenu/type/mode Git, ou contenu/type/mode POSIX |
| Valeur source et valeur référence | Empreinte SHA-256, type et mode, ou absence observée, ou inconnue motivée |
| Témoin Git | OID de commit/arbre et objet au chemin ; vérification OID avant utilisation ; absence justifiée par le parcours de l'arbre |
| Témoin checkout | Identité, métadonnées avant/après, intervalle et témoin du parent pour une absence ; aucune cible de lien suivie |
| Résultat de lecture | Succès stable ou lacune typée : mode manquant, accès, conflit, dérive, budget, objet/type non pris en charge |

Les empreintes de référence ne sont pas jetées après le classement. Le
validateur recalcule résultats, références correspondantes et compteurs à
partir des valeurs persistées ; il contrôle bindings, chemins et couverture.
Ce contrôle hors ligne établit une cohérence, pas l'authenticité d'une capture
entièrement falsifiée, une observation actuelle ou une sauvegarde durable.

`matches_target` exige une égalité Git ; `matches_reference` une égalité POSIX.
L'index ne fournit pas un mode POSIX complet : garder cette comparaison inconnue
lorsque ce mode est nécessaire. Ne pas la masquer comme « non applicable » pour
obtenir `different`. Une égalité prouvée peut survivre à une autre référence
inconnue ; la lacune et la couverture partielle restent visibles. `different`
exige toutes les lectures demandées pour la ligne et aucune égalité.

Par chemin : une couche différente prévaut ; sinon une inconnue prévaut ; sinon
toutes les couches égales à une cible donnent `matches_target` ; les autres
chemins entièrement comparés donnent `matches_reference`. Compter un chemin
une seule fois et conserver aussi les compteurs de couches.

### Couverture liée à la source

Conserver un registre par catégorie avec nombre source observé, nombre
d'observations émises, omises et inconnues, plus les témoins de parcours.
Réconcilier les compteurs du `LocalSummary` avec ces observations en définissant
explicitement les cas où un chemin contribue à plusieurs catégories ou stages.
Ne pas assimiler les huit échantillons à tous les changements.

`complete` est calculé, jamais accepté comme affirmation indépendante : source
HEAD/index/disque pertinente complète, chaque changement observé comptabilisé,
références inspectées, aucune omission, conflit ni lacune bloquante. Les trois
états « non demandé », « demandé partiellement observé » et « complet » sont
distincts. Sans détails disponibles, conserver les comptes positifs connus et
le manque de preuve ; ne pas inventer le nombre de chemins uniques manquants.

« Tous les changements comparés sont présents ailleurs » exige une couverture
complète et une égalité pour chaque couche pertinente. Même ce résultat ne
prouve pas la conservation des commits privés et ne satisfait pas C2 ou C3.

### Ordre des lectures et enveloppe globale

Terminer les phases de revue de base avant les lectures supplémentaires de
référence. Projeter les observations source lors des lectures existantes, dans
un buffer borné ; une saturation du buffer ne stoppe pas le collecteur de base.
Comparer ensuite avec le temps/octets/objets restant dans les allowances
applicables. La projection n'ajoute pas de coût de lecture invisible.

Un arrêt propre à la comparaison produit sa lacune et ne change pas
rétroactivement le résumé source. Conserver les contrôles de fin de capture sur
identités, index, refs et administration : une dérive invalide les preuves
affectées. Les observations source et référence portent leurs intervalles ;
elles ne deviennent pas un instantané atomique par partage d'un run.

Réserver les octets avant d'ajouter détails, témoins ou références. Utiliser
un plafond global fini et les plafonds existants de publication, avec
`bounded_json_size` en contrôle final. Prévoir des métadonnées minimales bornées
pour signaler les omissions. Si ces métadonnées ne tiennent pas, refuser le run
plutôt que publier un bilan complet sans sa comparaison. Ne pas tronquer un
`BytePath`, un OID ou un témoin pour faire tenir une ligne.

`run.json` inscrit taille et hash après écriture/synchronisation de l'artefact.
Son état partiel vaut : état partiel de base **ou** comparaison demandée
partielle. Feature désactivée : comportement existant. Un run partiel publié
garde la convention de succès existante ; liaison invalide et publication
échouée restent des erreurs. Texte, console et HTML reprennent les mêmes faits.

## Graphe et première livraison

| Unit | Delivers | Blocked by | Validation |
| --- | --- | --- | --- |
| U01 | Checkpoint et contrat vérifiable | Aucune dépendance technique | Préimages, restauration sur fixture sale, invariants et cas vides |
| U02 | Une cible Git de bout en bout | U01 | CLI, sidecar lié, reçu, texte/HTML ; positif et contre-exemples |
| U03 | Checkout explicite dans la même famille | U02 | Copies ignorées, mode POSIX, lecture bornée, identité et dérive |
| U04 | Toutes les cibles explicites, plusieurs familles, saturation du run | U03 | Preuve positive avec lacune ; mémoire/publication et reprises bornées |
| U05 | Assistant et guides exploitables | U04 | Canonique/projections, commandes documentées et limites visibles |
| U06 | Acceptation complète et revue indépendante | U05 | 14 cas, suites obligatoires, conservation et corrections |

Arêtes : U01 → U02 → U03 → U04 → U05 → U06. **Frontière technique : U01.**
Les unités sont séquentielles car elles partagent des fichiers. Aucun lancement
d'agent n'est implicite ; la méthode d'exécution reste à confirmer avec la v2.

La première livraison utilisable est **U02** : dans une famille temporaire, une
commande de revue montre le fichier égal à la cible, celui différent et les
inconnues dans le HTML et le texte. Une ligne permet de retrouver référence,
couche et preuve. Elle ne nécessite ni nouvel assistant ni nouveau moteur de
recherche. L'extension U03 n'entre qu'après passage de tous les gates U02.

## Phase 0 : U01, préservation et contrat

**Possession :** préimages dans un répertoire temporaire possédé, tests du
checkpoint et documents de contrat de ce lot. Pas d'édition de fichier produit
avant clôture des invariants ci-dessus.

- [ ] Relever branche/HEAD, chemins sales, hash et copies des fichiers touchés.
  Ne pas mettre de chemins privés ou de données personnelles dans les fixtures.
- [ ] Définir la comptabilité des catégories/couches/stages et le budget global
  à partir des plafonds existants. Documenter les états vide, non demandé et
  incomplet, la base des modes et l'ordre des captures.
- [ ] Construire et tester le checkpoint réversible décrit plus bas, sur une
  fixture déjà sale avec un fichier partagé et un fichier nouvellement créé.
- [ ] Inscrire les attendus des tests U02 avant le code ; aucun résultat produit
  réussi n'est déclaré au titre de ce checkpoint documentaire.

**Gate :** contrat sans ambiguïté sur le zéro complet, les références inconnues
et la conservation de preuves ; restauration exacte de la fixture, refus face
à une postimage changée et conservation du changement concurrent.

## Phase 1 : U02, une cible Git jusqu'au rapport

**Possession :** module dédié `src/local/comparison.rs`, projection facultative
dans `src/local/summary.rs`, orchestration `src/review.rs`, `src/main.rs`,
`src/run.rs`, publication `src/output.rs`, modèle séparé du contrat `review/2`,
rendus concernés dans `src/presentation/` et tests dédiés. L'intégrateur assure
les edits partagés. Créer un fichier seulement pour une responsabilité utilisée.

- [ ] RED : fixtures Git réelles avec un fichier identique à la cible, un octet
  différent et un index différent malgré un disque égal. Ajouter les cas sans
  changement complet et source incomplète. Le test exige des résultats utiles,
  pas seulement des refus.
- [ ] RED : fusion squash avec contenus équivalents sans ascendance, puis PR
  importée comme mergée avec un commit ou changement local supplémentaire.
  Le nouveau bilan n'efface ni historique privé ni conditions C1-C5 manquantes.
- [ ] Ajouter la projection source et le registre de couverture. Garder les
  catégories, index/disque et conflits ; aucun échantillon promu en liste totale.
- [ ] Lire une cible capturée par transport Git existant, sous le budget restant.
  Vérifier ses objets et refs. Comparer type/mode/contenu et absences prouvées.
  Types non supportés, intent-to-add et conflits restent inconnus dès U02.
- [ ] Publier le sidecar lié et le reçu, puis afficher les mêmes faits en texte,
  console et HTML statique. HTML sans JavaScript : bilan, chemins et preuves
  lisibles ; détails techniques repliables. Ne publier aucun octet de contenu.
- [ ] RED puis GREEN : positif à référence unique dont le classement seul, ou
  l'empreinte de référence seule, est altéré avec digest recalculé ; liaison à
  une autre revue, chemin hors source, faute d'écriture/synchronisation et petite
  enveloppe injectée. Refus ou
  couverture partielle explicite, jamais une fausse complétude.
- [ ] Vérifier `--worktree` et les anciens runs. Le worktree non demandé reste
  dans l'inventaire avec un bilan non collecté. Les filtres gardent la source.
  Jusqu'à U04, refuser clairement une demande multi-cibles non supportée par
  cette tranche ; ne pas sélectionner silencieusement la première ref.

**Tests proposés :** `tests/local_comparison.rs` et
`tests/test_local_comparison_cli.py`, réutilisant les helpers de fixtures et
l'oracle `filesystem_manifest`. Exécution ciblée depuis la racine :

```sh
rtk proxy cargo test --locked --offline --test local_comparison
python3.11 -B -m unittest discover -s tests -p 'test_local_comparison_cli.py' -v
```

**Gate :** tous les scénarios ci-dessus passent ; bilan positif non vide ; index
différent visible ; sidecar/reçu/rendus cohérents ; aucune mutation des repos
cibles. Le budget de comparaison seul épuisé conserve le résumé de base et rend
le nouveau bilan partiel. Aucun verdict existant changé sur corpus stable.

## Phase 2 : U03 et U04, étendre après validation

### U03. Checkout explicitement choisi

**Possession :** lecteur natif dédié si nécessaire dans `src/local/`, module de
comparaison, frontières existantes, raccordement CLI, rendus et tests U02.

- [ ] RED : fichier ignoré identique dans la copie fournie, puis unique sous
  `node_modules` ; configuration synthétique ne différant que par Node ou port.
- [ ] Comparer au même chemin avec mode POSIX, identités et métadonnées contrôlées.
  Une égalité checkout garde une provenance distincte d'une égalité Git.
  L'index sans mode POSIX requis reste inconnu. Ne pas suivre les symlinks.
- [ ] Tester chemin absolu/`..`, symlink intermédiaire, faux parent d'absence,
  mutation, dataless, gitlink, repo imbriqué et fichier spécial. Conserver les
  lacunes. Aucun chemin hors de la frontière fourni ne peut être lu.
- [ ] Refuser checkout étranger, auto-comparaison et checkout unique ambigu sur
  plusieurs familles. Si la copie est une ligne source du run, la garder visible
  avec son bilan non collecté, sans comparaison à elle-même.
- [ ] Vérifier fichiers cachés/ignorés, suppression face à absence et fichier
  vide, bit exécutable et lien différent. Secret synthétique absent des sorties.

**Gate :** positif et contre-exemple rendent respectivement égalité et différence
ou inconnue motivée ; aucune égalité POSIX inventée ; même `keep` pour ignorés.
Les contrôles de stabilité restent requis même en cas de réutilisation interne.

### U04. Toutes les références et enveloppe à l'échelle d'un run

**Possession :** orchestration multi-familles, budget global, validation/publisher,
rendus et fixtures dédiées. Les protections fondamentales existent déjà en U02.

- [ ] RED : plusieurs cibles, une égalité et une cible illisible ; plusieurs
  familles et chemins bruts longs, avec plafonds injectablement petits.
- [ ] Inspecter chaque référence demandée, conserver la preuve positive et sa
  lacune. Tous les échecs ne donnent pas `different`. Réutiliser seulement les
  objets/arbres vérifiés dans le run et revalider les refs/identités finales.
- [ ] Vérifier SHA-1/SHA-256, objets manquants/corrompus, non-UTF-8, HTML échappé,
  nombre de stages/couches et comptabilité de détails/omissions.
- [ ] Mesurer consommation maximale du buffer et taille publiée sur fixtures
  synthétiques. Tester dépassement de lignes, octets, objets et temps ; conserver
  les inconnues et une reprise par chemin exact. Ne pas relever les plafonds.
- [ ] Tester refus si métadonnées minimales trop grosses, succès partiel si
  détails omis, reçu partiel et absence d'un rapport final faussement complet.

**Gate :** aucun buffer ni artefact ne dépasse son enveloppe contrôlée ; témoins
entiers, statut et couverture cohérents ; preuve positive avec lacune observable.
Tous les cas multi-cibles auparavant refusés en U02 sont maintenant traités.

## Phase 3 : U05 et U06, parcours et acceptation

### U05. Consignes et assistant

**Possession :** `docs/contracts.md`, `docs/cli.md`, guides HTML/assistant,
`harness/skills/wt-sweep/`, `CHANGELOG.md` et projections par le générateur.

- [ ] Documenter options, artefact lié, mêmes faits JSON/texte/HTML, lecture des
  différences et reprise ciblée. « Même copie locale » ne signifie pas intégré.
- [ ] Faire vérifier liaison/couverture à la compétence canonique avant de
  proposer une action d'inspection. Garder les verdicts CLI ; pas de retrait
  exécutable ou de lecture de secrets.
- [ ] Charger `skill-creator` avant modification de la compétence, respecter
  ses gates applicables, puis générer les deux projections. Ne pas modifier
  directement `.agents/skills` ou `.claude/skills`.
- [ ] Tester workflow ancien et nouveau ; générateur en check ; vérifier liens
  et fixtures de documentation. Un validateur de fichiers ne prouve pas
  l'activation native d'une compétence.

**Gate :** instructions correspondent aux commandes livrées et aux inconnues ;
projections conformes ; aucune extension de la politique de retrait.

### U06. Recette et revue indépendante

**Possession :** tests d'intégration et dossier de preuves. Reviewer distinct
de l'auteur, en lecture seule et lancé seulement selon la méthode autorisée.

- [ ] Exécuter les 14 cas de la spec selon la matrice ci-dessous ; vérifier
  contenu/index/refs/config/hooks/objets avant et après avec oracle indépendant.
- [ ] Comparer revues avec/sans option sur corpus stable : inventaires,
  verdicts et faits locaux préexistants identiques. Couvrir la saturation
  propre à la comparaison ; ne pas attribuer une dérive réelle à cette règle.
- [ ] Exécuter les contrôles obligatoires ci-dessous. Conserver sorties, codes
  de retour, corpus et hashes de binaire ; ne pas déclarer un test prévu réussi.
- [ ] Faire relire intégration, omissions, preuves, budgets, frontières et HTML
  rendu. Corriger les findings puis retester les comportements concernés.

```sh
rtk proxy cargo fmt --all -- --check
rtk proxy cargo clippy --all-targets --locked --offline -- -D warnings
rtk proxy cargo test --locked --offline
rtk proxy cargo build --locked --offline
python3.11 -B -m unittest discover -s tests -p 'test_*.py' -v
python3.11 -B -m unittest discover -s experiments/exclusion -p 'test_*.py' -v
python3.11 -B scripts/sync-ai.py --check
```

**Gate final :** première commande de fixture produit un bilan utile et lié,
chaque ligne explicable, aucun faux zéro complet, inconnues fidèles, aucun
changement de C1-C5, aucune mutation cible ; suites passantes et findings
bloquants corrigés. Le résultat ne prouve ni performance réelle à grande échelle,
ni nettoyage autorisé, ni publication.

## Matrice d'acceptation

| Cas de la spec | Première couverture | Clôture |
| --- | --- | --- |
| 1. Égalité / octet différent | U02 | U06 |
| 2. Index différent / disque égal | U02 | U06 |
| 3. Squash sans preuve d'ascendance | U02 | U06 |
| 4. PR mergée et travail supplémentaire | U02 | U06 |
| 5. Ignoré égal / ignoré unique | U03 | U06 |
| 6. Config différente et secrets non publiés | U03 | U06 |
| 7. Suppression, vide, modes, liens | U02/U03 | U06 |
| 8. Formats Git, corruption, chemins bruts | U02/U04 | U06 |
| 9. Dérive, index spécial, types refusés | U02/U03 | U06 |
| 10. Plusieurs cibles, positif et lacune | U04 | U06 |
| 11. Plafonds et reprise ciblée | U02/U04 | U06 |
| 12. Liaison, publication, ancien run | U02/U04 | U06 |
| 13. Non-mutation indépendante | Dès U02 pour chaque unité | U06 |
| 14. Verdicts stables et analyse utile | Dès U02 pour chaque unité | U06 |

## Retour arrière vérifiable, pour chaque unité

U01 prépare un répertoire temporaire possédé par unité avec préimages, listes
de fichiers créés et postimages hashées. Il prépare et teste `rollback.py` dans
ce répertoire ; ce script n'est pas un nouvel outil produit ni disponible
avant ce lot. Commande de restauration prévue, une fois le checkpoint créé :

```sh
python3.11 "$unit_checkpoint/rollback.py" --verify-and-restore
```

Le script vérifie **toutes** les postimages et frontières avant d'écrire, restaure
les seules préimages possédées et retire un fichier créé seulement si son hash
est toujours celui enregistré. Aucun lien suivi ni répertoire partagé supprimé.
Il vérifie ensuite les hashes restaurés et les témoins de changements préexistants.
Il ne restaure pas `HEAD` et ne fait pas de reset Git. Une modification concurrente
fait refuser l'automatisme ; conserver le checkpoint et réconcilier les hunks
concernés avant toute nouvelle restauration.

**Preuve :** test positif de restauration exacte d'une fixture sale ; test avec
un edit concurrent qui refuse sans modifier ce fichier ni les autres fichiers.
À chaque unité, conserver le diff explicite et les hashes avant/après ; pas de
staging global ni commit implicite.

## Déclencheurs, arrêts et remise

Une unité suivante commence uniquement après tous les gates de la précédente.
La revue peut produire un bilan partiel justifié ; elle ne relâche pas la
couverture pour atteindre un objectif de candidats. Si les preuves ne tiennent
pas dans les allowances, rendre le manque observable et proposer le ciblage
existant avant toute nouvelle optimisation.

Arrêter le lot concerné en cas de lecture hors frontière, perte de bytes de
chemin, égalité sans témoin, C1-C5 changé, dépassement non borné ou préimage
concurrente menacée. Erreur d'accès ou objet manquant : inconnue explicite,
pas abandon silencieux. Toute nécessité de modifier `review/2`, d'ajouter une
dépendance, réseau ou politique de retrait demande une nouvelle décision de
périmètre ; aucun de ces changements n'est implicite dans cette v2.

Extensions différées : recherche d'équivalents ailleurs que le même chemin,
normalisation de configurations, classement « cache jetable », nouvelle
navigation HTML ou politique de suppression. Déclencheur : cas utilisateur
reproduit, insuffisance du bilan démontrée et périmètre approuvé séparément.

Remise : fichiers changés, tests observés, limites restantes, preuves de
conservation et findings de revue. Ce plan ne lance ni implémentation, agents,
commit, push, publication ou retrait réel.
