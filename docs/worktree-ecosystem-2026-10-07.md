# Gestion des worktrees : comparer avant d'élargir Sweep

[Index documentaire](README.md) · [Feuille de route](roadmap.md) · [Recherche précédente](references.md)

Recherche du 07/10/2026, depuis Agentic Ecosystem Map, Perplexity et les sources
publiques des projets. Les concurrents n'ont pas été installés ni exécutés.
Ce document recommande un périmètre ; il ne livre aucune intégration.

Suite du 07/10 : les 12 projets ont depuis été clonés et leur code analysé dans
le [comparatif d’implémentation](worktree-source-analysis-2026-10-07.md).
Il précise les mécanismes et les licences ; aucune exécution produit n’a été faite.

## Recommandation

Tester **Holt et AgentRinse avant d'ajouter la gestion et la maintenance**.
Holt recouvre aussi l'audit et le rapport visuel envisagés pour Sweep. La
création, la navigation et les sessions sont déjà couvertes par Worktrunk,
kwt, gwq et Workroot. Un nouveau gestionnaire généraliste apporterait peu de
valeur sans lacune mesurée chez ces outils.

Conserver le moteur actuel et ses fixtures pendant cette comparaison. Le
rapport HTML reste utile comme vue des résultats de Sweep, mais son existence
ne suffira pas à différencier le projet. La valeur à vérifier est la combinaison
de la couverture déclarée, des preuves de conservation, des mesures disque et
de la restitution entre repos. Elle n'est pas démontrée comme exclusive.

## Concurrents directs

Les fonctions ci-dessous sont **documentées par leurs auteurs**, pas validées
par une exécution de notre côté. Confiance élevée sur le recouvrement annoncé,
comportement sur nos fixtures `UNKNOWN` pour chaque candidat.

| Projet et source primaire | Fonction qui recouvre Sweep | Limite ou différence à tester |
| --- | --- | --- |
| [Holt](https://github.com/Raed2180416/holt) | Travail unique entre worktrees, preuves de conservation, quarantaine/restauration, intégrations d'agents et graphe hors ligne. | Le périmètre free/core publié est mono-repo. Le code de `main` peut dépasser la release. La [matrice de preuves](https://github.com/Raed2180416/holt/blob/main/docs/FEATURE-PROOF-MATRIX.md) référence l'export HTML et ses tests, que nous n'avons pas rejoués. |
| [AgentRinse](https://github.com/vincentkoc/agentrinse) | Inventaire de résidus d'agents, liens avec Codex/Claude, plan adressé par contenu, revalidation et actions de nettoyage. | La [récupération](https://github.com/vincentkoc/agentrinse/blob/main/docs/recovery.md) utilise une quarantaine et des refs de récupération. Elle refuse notamment les états ignorés/non suivis dans son contrôle de restauration. Ce n'est pas une preuve de sauvegarde autonome de tout état. |
| [Stalewood](https://github.com/retif/stalewood) | Parcours de répertoires, inscriptions Git et worktrees abandonnés ; taille, JSON groupé par repo, mode lint et retrait. | Intégration fondée sur l'accessibilité des commits ; une autre branche peut contenir le travail. Comparer la cible choisie et les dépendances entre candidats. Pas de restauration complète établie ici. |
| [gh-reaper](https://github.com/ai-ecoverse/gh-reaper) | Découverte transversale, âge, taille, état Git, JSON et nettoyage explicite. | Exclusions de parcours documentées ; changements de lockfiles ignorés par défaut, option de PR fusionnée distincte de l'ascendance. Ces règles diffèrent du contrat de Sweep. |

La [matrice de Holt](https://github.com/Raed2180416/holt/blob/main/docs/FEATURE-PROOF-MATRIX.md)
distingue tests de contrat et activation réelle des hôtes. Elle signale aussi
que les ACL et attributs étendus ne sont pas représentés par les objets Git.
Ses hooks ne couvrent pas les processus externes arbitraires. Ces limites
comptent pour notre exigence de restauration et pour E1 ; elles ne constituent
pas, à elles seules, un défaut de son contrat annoncé.

AgentRinse documente un déplacement atomique de quarantaine sur le même système
de fichiers. La quarantaine conserve l'occupation disque ; la purge est une
action séparée. C'est une piste à comparer au coût des archives contenant des
copies de dépendances. Elle ne démontre ni un gain APFS ni une restauration
indépendante après disparition du repo source.

## Gestion courante, registre et sessions

| Projet et source primaire | Fonction disponible selon sa documentation | Conséquence pour Sweep |
| --- | --- | --- |
| [Worktrunk](https://github.com/max-sixty/worktrunk) | Création, changement de worktree, liste, merge, retrait, hooks et intégrations d'agents. | Candidat prioritaire pour le cycle de vie. Sa [politique de retrait](https://worktrunk.dev/remove/) accepte aussi arbre identique, simulation de merge ou patch-id : ne pas convertir son verdict en preuve C1-C5. |
| [kwt](https://github.com/kenn-io/kwt), [contrats](https://kwt.sh/docs/reference/cli/) | Registre de projets, inventaire JSON, génération de worktree, état local et sessions tmux. | Candidat pour registre et liens de sessions. Le périmètre porte sur les projets inscrits, pas sur toute la machine. `created_at` est documenté comme mtime du dossier, pas comme preuve de création. |
| [gwq](https://github.com/d-kuro/gwq) | Création, navigation, retrait, tableau de statut, JSON/CSV et tmux. Découverte dans un répertoire de base configurable ; la lecture du code confirme aussi un registre de métadonnées. | Alternative CLI à évaluer si une convention de rangement suffit. Son mode global ne prouve pas une découverte exhaustive de la machine. |
| [Workroot](https://github.com/fridiculous/workroot) | Découverte, statut transversal, cibles nommées, création, exécution, push et prune ; sorties JSON. | Recouvrement direct avec une extension « gestion ». Aucun protocole équivalent à notre sauvegarde complète n'a été établi ici. |
| [Git Worktree Runner](https://github.com/coderabbitai/git-worktree-runner) | Création par branche, préparation du workspace, copie de configuration, éditeur et agent ; liste `--porcelain`. | Réutilisation possible pour préparer un environnement. Ce rôle ne remplace pas l'audit des données à conserver. |

Le [site de kwt](https://kwt.sh/) décrit un daemon propriétaire du registre et
des sessions. En dépendre aurait donc un coût d'architecture pour Sweep, dont
la CLI actuelle n'exige pas de service. Un import explicite de résultats JSON
serait un premier essai plus petit qu'une intégration de son runtime.

Worktrunk dispose d'une [extension par exécutable](https://worktrunk.dev/extending/#custom-subcommands) :
`wt-sweep` placé sur `PATH` peut être invoqué comme `wt sweep`, sous réserve des
commandes intégrées et alias prioritaires. Le mécanisme est documenté ; la
compatibilité réelle avec notre CLI reste à tester. Aucun hook de retrait
automatique n'est proposé par cette recherche.

## Projets adjacents et faux positifs

- [Worktree Manager](https://github.com/guoyongchang/worktree-manager) couvre
  les workspaces multi-repos, terminaux et MCP. Son archivage masque un
  worktree en conservant ses fichiers ; cela ne libère pas son espace disque.
- [Superset](https://github.com/superset-sh/superset) est un environnement
  d'orchestration d'agents. Il recouvre une éventuelle application de pilotage,
  sans établir ici un remplacement de nos preuves de conservation.
- [claude-cow-worktree](https://github.com/palmin/claude-cow-worktree) utilise
  `clonefile()` pour créer des worktrees sur un même volume APFS. C'est une
  piste sur le coût de création, pas un inventaire ni une sauvegarde autonome.
- Le corpus retourne `holt-os/holt` sur « Holt » : c'est un agent personnel,
  pas `Raed2180416/holt`. Les occurrences statistiques « Holt-Winters » sont
  également hors sujet. Les skills CMS, listes de liens et orchestrateurs
  mentionnant seulement `git worktree` ne sont pas des concurrents directs.

Aucun candidat n'est classé abandonné : les dates d'activité ne suffisent pas
et les métadonnées GitHub consultées ne les déclarent pas archivés. Une entrée
dans le fichier de tri `cimetiere.csv` du corpus n'est pas une preuve d'abandon.

## Traçabilité et licences

`pushed_at` est une observation GitHub du 07/10, pas une date de release, une
preuve de maintenance future ou une validation du code. La catégorie de
collecte décrit la requête ayant trouvé le repo dans Ecosystem Map. `UNKNOWN`
signifie qu'aucune catégorie canonique complète n'a été retenue par le helper.

| Repo | Catégorie collecte / canonique | Dernier push UTC observé | Licence déclarée |
| --- | --- | --- | --- |
| Raed2180416/holt | absent du corpus | 2026-10-06 | FSL-1.1-MIT hors `src/team`, licence commerciale dans `src/team` |
| vincentkoc/agentrinse | harness / UNKNOWN | 2026-10-05 | MIT |
| retif/stalewood | absent du corpus | 2026-08-12 | MIT |
| ai-ecoverse/gh-reaper | absent du corpus | 2026-08-29 | Apache-2.0 |
| max-sixty/worktrunk | transverse / UNKNOWN | 2026-10-07 | MIT OR Apache-2.0 |
| kenn-io/kwt | absent du corpus | 2026-10-06 | Apache-2.0 |
| d-kuro/gwq | factory / UNKNOWN | 2026-10-02 | Apache-2.0 |
| fridiculous/workroot | harness / UNKNOWN | 2026-07-29 | MIT |
| coderabbitai/git-worktree-runner | absent du corpus | 2026-09-15 | Apache-2.0 |

Sources : API publique `GET /repos/{owner}/{repo}`, puis
[Cargo.toml Worktrunk](https://github.com/max-sixty/worktrunk/blob/e4a25d6e37cc7bfa6d133ba625359c58fd3cda08/Cargo.toml)
et [licence Holt](https://github.com/Raed2180416/holt/blob/b99e098e214f4e84388ffa9d6fa6b675f951732a/LICENSE.md)
pour les deux valeurs `NOASSERTION` de GitHub. La FSL de Holt comporte une
restriction de « Competing Use » : ne pas traiter son code comme une dépendance
MIT immédiatement réutilisable. Le tableau n'est pas une analyse de
compatibilité de licences.

Têtes `main` observées : Holt `b99e098e214f4e84388ffa9d6fa6b675f951732a`,
AgentRinse `6d5e047e00ef0193f6b75b952060a80ea310b1a4`, Worktrunk
`e4a25d6e37cc7bfa6d133ba625359c58fd3cda08`. La dernière release Holt retournée
par l'API est [v0.4.7](https://github.com/Raed2180416/holt/releases/tag/v0.4.7),
publiée le 08/09/2026. Son artefact n'a pas été comparé au code lu.

## Vérifications à lancer avant de choisir

| Essai sur repos de test temporaires | Observation attendue | Décision permise |
| --- | --- | --- |
| Même corpus pour Sweep, Holt et AgentRinse | Cas propre intégré positif ; modifications suivies, index distinct du disque, fichiers non suivis/ignorés, stash et reflog seuls. Comparer les raisons, lacunes et octets conservés. | Remplacer une fonction seulement si son besoin et son contrat sont couverts. Un refus systématique ne suffit pas. |
| Découverte Sweep / Stalewood / Workroot | Repos imbriqués, worktrees inscrits hors racine, alias, chemins inaccessibles et métadonnées orphelines. | Choisir une découverte existante si sa couverture et ses exclusions conviennent. |
| Quarantaine/restauration Holt / AgentRinse | Interruption, changement après validation, processus d’écriture avec descripteur déjà ouvert ; oracle indépendant HEAD/index/fichiers/modes. Essai séparé sans repo source pour une sauvegarde autonome. | Distinguer récupération locale et U5 ; aucune levée d'E1 par simple présence d'un verrou. |
| Rapport et performances | Même fixture de 50 puis 100 worktrees ; résultats complets/partiels, temps médian, mémoire, coût des ignorés, lisibilité du rapport. | Choisir entre adopter l'interface existante et rendre nos propres artefacts. |
| Gestion Worktrunk / kwt | Création et inscription explicites, import JSON, remplacement au même chemin, session terminée, conservation des configs étrangères. | Ajouter un adaptateur borné plutôt qu'un second registre propriétaire si cela répond au besoin. |

Ces essais sont proposés, **pas exécutés dans cette recherche**. Les versions
doivent être figées et les configurations isolées avant exécution. Une
performance publiée par l'auteur n'est pas un benchmark de notre machine.

Si Holt couvre le besoin d'audit et de restitution sur ces fixtures, envisager
son adoption ou une contribution avant de poursuivre ce périmètre dans Sweep.
Si un écart précis reste ouvert, le documenter avec son cas reproductible et
implémenter seulement ce complément. Importer `.worktree.json` peut rester
utile comme provenance ; une carte ou un index périmé ne doit pas établir
l'absence d'usage courant.

## Recherche reproductible et limites

Le helper `agentic-project-finder/scripts/search_projects.py` a interrogé
`data/corpus.sqlite` d'Agentic Ecosystem Map en lecture seule. Deux recherches
larges ont croisé `worktree|worktrees` avec gestion, création, navigation,
registre, nettoyage, découverte, disque et restauration. Les requêtes excluaient
« awesome list » et « tutorial only ». Une troisième a ciblé les expressions
« worktree manager », « worktree cleanup » et les noms des premiers candidats.
Une quatrième a recherché Holt, puis les identités exactes ont été vérifiées.

Les quatre sorties sont `partial` : certaines voies lexicales sont plafonnées
à 200, 500 ou 800 résultats. Les listes finales étaient limitées à 25, 25, 40
et 10 entrées. Il ne s'agit pas d'un recensement exhaustif. Aucun README n'a
été ajouté au corpus et aucun moteur d'embeddings n'a été lancé.

Perplexity Deep Research a échoué avec `TypeError: terminated`. Trois recherches
Perplexity Search ont ensuite abouti : audit/nettoyage, registre/sessions, puis
`"Holt" "worktree" cleanup`. Elles ont notamment remonté la documentation de
kwt et les références à Holt. Les conclusions reposent sur les sources
primaires liées dans ce document, pas sur une synthèse générée seule.

L'absence dans le corpus ne signifie pas absence du marché. Les requêtes GitHub
biaisent la collecte, les README sont inégalement indexés et les métadonnées
locales peuvent dater. Les produits commerciaux fermés sont moins représentés.
Les audits automatiques ne sont pas une validation humaine. Aucune enquête
d'adoption, installation des concurrents, comparaison runtime ou qualification
de leur nettoyage sur les repos personnels n'a été menée.
