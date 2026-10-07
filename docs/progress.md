# Résultats d'implémentation

[Index documentaire](README.md) · [Contrats des artefacts](contracts.md) · [Feuille de route](roadmap.md)

Le [dernier checkpoint du lot audit](#audit-lisible--intégration-du-07102026)
regroupe les validations HTML, collecte et assistant. Les sections précédentes
conservent les observations historiques et leurs limites.

## Reprise du 07/10/2026

Le checkout de travail précédent était absent. L’instantané Git
`14f00ea454795fd6df1f86dec72ec6573d3e546e`, créé par Codex sous le libellé
`startup-cleanup`, conserve les ajouts disque et métriques. La branche
`codex/html-reports` reprend cet instantané dans un checkout isolé. Le dossier
principal et ses modifications locales sont préservés.

Les 24 tests CLI, disque et jugement rejoués sur cet état passent. Ce passage
ne remplace pas la suite complète du 06/10 et ne résout pas l’écart du test à
100 worktrees. Cet état de reprise précède le lot d’audit lisible décrit plus
bas : HTML implémenté et cause de l’écart de collecte reproduite.

Le contrôle GitHub du 07/10 confirme les PR 1 à 7 fusionnées, aucune PR ouverte
et `main` à `31b58339c6d4a84750bbbad2c0f89ebfa4c8624f`. Les ajouts disque et
les dernières métriques restent dans l’instantané local, sans PR de publication.

## Affichage terminal, 07/10/2026

La CLI met à jour une seule ligne de progression dans un terminal compatible, en respectant sa largeur.
Une redirection conserve les étapes principales. `--summary` affiche les
verdicts, la couverture et les tailles ; `--verbose` conserve les événements
par dossier et les métriques détaillées. Les couleurs et icônes sont automatiques,
avec `--color auto|always|never`, `NO_COLOR` et `TERM=dumb`. Les mesures partielles
restent signalées et les contrôles manquants du bilan court portent sur les
lignes sélectionnées. Le rapport complet et les artefacts JSON restent sans
décoration. La collecte et les règles de jugement ne sont pas modifiées.

Les changements portent sur `src/main.rs`, `src/terminal.rs` et le rendu dans
`src/presentation/`. Les README anglais et français, la référence CLI, le tutoriel,
le guide disque et le changelog documentent les options. Les tests utilisent des
dépôts jetables ; aucun scan de la machine ou des dépôts personnels n’a été relancé.

Les six scénarios initiaux d’affichage échouent avant implémentation puis passent :
progression compacte, récapitulatif court, mode détaillé, codes de couleur absents des
artefacts, détection indépendante des deux terminaux et désactivation de la
décoration. Les pseudo-terminaux sont réels. Deux tests du rendu couvrent les
mesures partielles, les filtres et les conditions manquantes. Sur la même fixture
à deux worktrees, la progression redirigée passe de 18 à 6 lignes ; le nouveau
bilan mesure 24 lignes sans dossier de publication.

Le passage Rust complet termine avec **342 tests réussis, 3 ignorés**. La suite
Python exécute **284 cas, 6 ignorés, un échec** : le test de publication attendait
le titre supprimé `CHECKS STILL NEEDED` alors que les deux verdicts étaient résolus.
Ses assertions suivent désormais le bilan compact, avec les mêmes verdicts et
la même couverture attendus. Les scénarios d’ordre sur 46 et 100 worktrees passent
lors de ce rejeu ; ce passage seul ne démontre pas la disparition de l’écart
intermittent observé le 06/10.

Un test supplémentaire reproduit ensuite le repli de la progression à
32 colonnes. Le correctif lit la largeur courante via la fonction `termios` de
`rustix`, déjà présent dans le projet. Après cet ajustement, **31 tests Rust
ciblés et 9 tests Python passent** : les huit tests d’affichage et le test de
publication corrigé. Les essais de largeur couvrent 2, 4, 32 et 80 colonnes,
ainsi qu’une largeur non renseignée. La ligne est effacée avant un diagnostic
d’erreur. Les suites globales ne sont pas intégralement rejouées après cet
ajustement d’affichage.

Le formatage, Clippy, la compilation release, les 22 projections d’instructions
et les 132 liens locaux contrôlés dans les documents modifiés passent. Les
changements restent locaux, sans commit ni publication GitHub pour ce lot.

## État courant au 06/10/2026

U1-U4 et T01-T09 sont livrés en lecture seule. Le triage a rejoint `main` par
la [PR #1](https://github.com/FlorianBruniaux/wt-sweep/pull/1), commit `ddda77d`.
`review` regroupe découverte et revue ; le rapport distingue `keep`,
`cleanup_candidate` et `verify`, avec cibles, faits locaux, histoire et conditions
manquantes. Le [checkpoint T01-T09](finish-ledger.md#triage-t01-t09) contient les
vérifications finales et l'essai réel : 48 worktrees, 3 `keep`, 45 `verify`, aucun
candidat. Ce résultat historique précède les profils de collecte décrits ci-dessous.

La [livraison GitHub](finish-ledger.md#livraison-github-du-06102026) sépare la CI
réussie avant fusion de la vérification du commit fusionné. La
[feuille de route](roadmap.md) garde le diagnostic de performance prioritaire.
E1 reste non démontrée et `apply` indisponible.

## Espace disque, 06/10/2026

Le checkout local ajoute `scan --disk-usage`, `review --disk-usage` et
`--disk-budget standard|thorough`. Le rapport final et le récapitulatif console
rendent les mesures capturées directement depuis la CLI. `plan` les conserve
avec leur intervalle d’origine ; les filtres gardent les totaux de la source.
Une absence de mesure est affichée explicitement.
Le collecteur mesure les métadonnées par descripteur, sans lire le contenu des
fichiers. Les tailles allouées et logiques sont publiées par dossier de travail
et administration Git, avec total dédupliqué par périphérique/inode. Les autres
racines enregistrées imbriquées sont comptées séparément. Le tri du rapport par
taille conserve branche, chemin et dépôt ; les lacunes restent explicites.
Le [guide disque](disk-usage.md) précise les plafonds et la limite APFS.

Les sept scénarios initiaux du collecteur passent : fichiers ignorés et creux, liens symboliques,
liens physiques locaux et partagés, racines imbriquées, budgets, racine absente ou
remplacée, noms avec caractères de contrôle et dépôt nu. Les neuf tests CLI
passent, y compris le refus d’un profil invalide et l’acceptation de `thorough`.
Une première fixture utilisait un nom non UTF-8 refusé par APFS ; elle utilise
maintenant tabulation et saut de ligne. Ce contrôle ne prouve pas la gestion
native de noms non UTF-8 sur un autre système de fichiers.

Avant l’intégration au rapport de revue, la suite Rust exécutée avec un seul test à la fois passe : 336 tests,
plus trois ignorés. Une exécution concurrente avait dépassé le délai du test de
transport de 32 Mio ; son rejeu isolé passe en 5,66 s sans modifier le seuil.
La suite Python complète a exécuté 277 cas, avec six ignorés et deux échecs de
sous-scénarios dans le test d’ordre sur 46 et 100 worktrees. L’écart porte sur
`cleanup_candidate` contre `verify` entre deux collectes. Ce passage n’est pas
vert. La charge de la machine et des collectes concurrentes était élevée ; cela
ne suffit pas à attribuer la cause. Le rejeu isolé termine en 443,879 s : le
sous-scénario à 46 worktrees passe, celui à 100 conserve l’écart de verdict.
Ce constat reste ouvert ; la validation Python globale n’est pas acquise.
Les tests et les plafonds n’ont pas été modifiés pour contourner cet échec.
Formatage, Clippy et build release passent à ce checkpoint. Le SHA-256 de ce binaire release
est `3bd01ef53f49d386edbbba999f7c5ceab8d8654c1dedf935a0896e32705d61db`.

Les README, aide CLI, contrats, changelog, index et roadmap sont alignés. Les
22 projections d’instructions restent synchronisées. Les 179 liens locaux et
sept blocs shell contrôlés sont valides. La publication GitHub reste à faire.
Les mesures personnelles restent hors du dépôt.

### Raccordement au rapport final

`review` accepte la mesure dans la même commande que l’analyse. Les tests ajoutés
vérifient la publication native des tailles dans la console, le texte et le JSON,
leur conservation par `plan` après modification des fichiers, le même rendu pour
les mêmes observations et l’absence d’effet sur un verdict positif de candidat.
Une vue filtrée conserve toutes les mesures et n’affiche le bilan qu’une fois.
Les métriques précisent que la mesure disque a sa propre durée.

La suite Rust complète passe avec 340 tests et 3 ignorés. Les deux derniers
libellés de temps ont ensuite été revérifiés par 26 tests ciblés du rendu, du
jugement et de la mesure. Les 32 scénarios Python de revue et de publication
passent également. Formatage, Clippy et build release
passent. Un essai release sur deux dossiers de travail et une administration Git
jetables retrouve les octets alloués de l’oracle `stat` et publie les mêmes données
dans `inventory.json` et `review.json`. Le binaire de ce raccordement a pour SHA-256
`dec98c6dea3accf9fe74b009eeb9b25fc5ff731b431a5cfb09659985182d2826`.
Ces contrôles n’effacent pas l’échec d’ordre à 100 worktrees décrit plus haut.

## Indicateurs de performance, 06/10/2026

Le checkout local `codex/scan-performance` ajoute durée monotone, tentatives de
lecture de dossiers, entrées listées et workers dans les métriques de découverte.
La console affiche le débit et la couverture séparément. Les revues ajoutent le
débit local, les appels Git mesurés et la part de la phase la plus longue.
Le [benchmark Rust](collection-performance.md#benchmark-rust-reproductible)
compare les workers sur une fixture constante avec vérification de conservation.

Les tests ont d’abord échoué sur l’absence de métriques et leur rejet par le
lecteur, puis passé après raccordement. La suite complète compte 327 tests Rust
réussis et trois ignorés, dont le benchmark exécuté séparément avec succès.
La suite Python 3.14 compte 277 cas : 271 réussis et six ignorés. Une première
invocation avec Python 3.9 a échoué sur `hashlib.file_digest` et a été interrompue ;
elle ne fait pas partie du résultat de validation. Formatage, Clippy et contrôle
des 22 projections passent. Les 373 liens documentaires locaux existent et les
six blocs shell des README et guides modifiés sont syntaxiquement valides.

La mesure isolée donne une médiane de 1,043 s à un worker et 1,157 s à quatre,
pour 2 218 entrées avec couverture complète dans les deux cas. Les plages se
chevauchent ; aucun gain du parallélisme n’est établi. Le
[registre des mesures](collection-performance.md#mesure-locale-du-06102026)
conserve le corpus, les empreintes et les limites. Aucun scan global réel ni
publication distante n’a été exécuté pour ce lot. Les 60 secondes de découverte
et l’absence de reprise restent des limites distinctes.

## Profils et grands scénarios A1-A6, 06/10/2026

Le lot ajoute les profils finis `standard|thorough`, les plafonds appliqués et
les métriques par phase. Les vérifications approfondies groupent les blobs ;
l’histoire privée partage son parcours et donne 32 expansions prioritaires aux
ancres déjà utiles. Le transport Git attend les descripteurs disponibles au
lieu de temporiser entre chaque petite rafale de sortie. C1-C5 restent inchangées.

Le scénario synthétique de 4 200 blobs distincts, 600 commits et 32 racines privées
obtient un candidat complet. Une famille de 50 worktrees donne 48 candidats et
deux rétentions justifiées, avec conservation indépendante de la source. Sur
une sonde constante de 64 Mio répartis entre 16 enfants Git, la collecte passe
de 46 540 ms à 500 ms. Ce résultat de transport ne mesure pas la revue complète.

Le nouveau passage réel ciblé établit une ascendance, couvre les deux racines
privées et classe le worktree `keep` : une modification non commitée et au moins
33 741 fichiers ignorés sont observés. Les 52 213 chemins examinés ne constituent
pas un inventaire complet ; le plafond local d’octets laisse C2, C3 et C5
incomplètes. La phase privée dure 5 063 ms sur ce passage, contre 240 473 ms sur
le diagnostic précédent. Le dépôt évolue entre les deux observations.

Les README, contrats, tutoriel, aide CLI, roadmap et skill expliquent le passage
ciblé et les limites restantes. Les projections Claude/Codex sont synchronisées ;
le routage BM25 est vérifié sur les deux clients. Le chargement dans une nouvelle
session native reste non vérifié. Le [registre de collecte](collection-performance.md)
conserve les observations distinctes et leurs identités de binaire.

Le passage familial thorough conserve les 50 lignes : 49 `keep`, un `verify`,
aucun candidat. L’histoire des commits est collectée pour les 50 ; dix résumés
locaux sont complets et quarante restent partiels. Les phases locale et privée
atteignent leurs plafonds. Les 49 rétentions ont des motifs observés, dont des
fichiers ignorés ; elles ne signifient pas 49 analyses exhaustives. Le dernier
`verify` garde notamment un index intent-to-add non pris en charge et une
histoire privée incomplète. Les artefacts du run sont vérifiés contre leur reçu.

La revue indépendante est statique : elle couvre C1-C5, les budgets, les témoins
privés, l’attente des flux Git et la conservation des causes d’arrêt. Aucun
constat de cette revue ne reste ouvert. Formatage et Clippy passent. La suite
Rust finale compte 324 réussites et deux cas ignorés ; la suite Python compte
271 réussites et six cas ignorés. Les 6 expériences d’exclusion passent,
sans démontrer E1. Le binaire release est reconstruit ; le
[registre de validation](finish-ledger.md#complétion-des-gros-dépôts-a1-a6)
conserve son empreinte et les limites des contrôles.

## Collecte des gros dépôts, 06/10/2026

Les lectures locales d’arbres et de blobs sont groupées et dimensionnées avant
exécution. Le graphe d’une cible vérifiée est partagé entre worktrees de la même
famille physique. Le temps local restant est réparti entre les worktrees ;
`--worktree` permet un passage concentré sur un chemin, tout en conservant
l’inventaire et les omissions explicites. Les README, le tutoriel, les contrats,
l’aide CLI et les instructions Claude/Codex générées décrivent ce comportement.

Les fixtures distinctes passent avec 23 processus Git pour 1 024 blobs et au plus
40 lectures d’objets pour 1 024 dossiers. Un graphe partagé de 12 commits demande
12 expansions. Les délais de phase d’histoire et d’histoire privée n’effacent
plus les preuves indépendantes comme le ferait un vrai timeout du backend.
Les revalidations manquantes restent inconnues.

Le dernier passage complet compte 49 worktrees : 4 `keep`, 45 `verify`, aucun
candidat. Il conserve trois ascendances vérifiées, sans fausse quarantaine ;
les 49 résumés locaux restent partiels. Le passage ciblé vérifie une ascendance
et observe au moins 4 969 fichiers ignorés, donc classe ce worktree `keep`. Les plafonds d’objets et d’histoire privée
laissent encore des contrôles incomplets. Le [registre de collecte](collection-performance.md)
détaille les fixtures, la revue indépendante et les limites des mesures réelles.

Validation du lot : 303 tests Rust réussis, 2 ignorés ; 260 tests Python réussis
sur 266, 6 ignorés ; six expériences de conservation réussies. Formatage, Clippy,
builds et synchronisation des 22 fichiers générés passent. Le contrôle
documentaire couvre 31 fichiers, 327 liens locaux et 38 blocs shell. La revue
indépendante est statique ; les suites intégrées ont été exécutées par
l’intégrateur. La validation distante reste distincte de ces résultats locaux.

## Bilan console, 06/10/2026

`--summary` affiche les verdicts, la couverture du périmètre source, le nombre de
contrôles locaux complets, partiels ou absents et les conditions encore manquantes.
Les budgets d’historique, budgets locaux et timeouts Git sont regroupés. Les
messages de progression sont lisibles ; le JSON et les règles de décision restent
inchangés. Ce changement d’affichage ne résout pas les limites de performance
observées sur le gros dépôt.

Vérifications locales : 280 tests Rust réussis, 2 ignorés ; formatage, Clippy,
synchronisation des 12 fichiers d’instructions et 6 tests RETEX réussis.
La suite Python complète a exécuté 253 cas : 246 réussis, 6 ignorés et une ancienne
assertion `processed=1` à mettre à jour. Après cette correction et l’accord des
messages au singulier, les 9 tests du module concerné passent. La suite Python
complète n’a pas été rejouée après ces derniers changements.
Le rendu du rapport existant de 50 worktrees donne 3 à conserver, 0 candidat et
47 à vérifier, avec 0 contrôle local complet, 4 partiels et 46 non collectés.
Aucune nouvelle analyse du dépôt réel n’a été lancée pour cette vérification.

## Skill wt-sweep, 06/10/2026

Le [skill utilisateur](assistant-skill.md) est intégré au harnais du dépôt.
Il vérifie l’aide CLI, choisit le périmètre et les cibles explicites, lit les
rapports par pages et propose des actions de conservation ou de vérification.
Les README anglais/français, le tutoriel et la feuille de route le référencent.
`apply` reste indisponible ; le skill ne produit pas de suppression de remplacement.

La source `harness/skills/wt-sweep` est projetée à l’identique dans les dossiers
natifs Claude et Codex. Les 20 fichiers générés sont synchronisés. Les tests du
générateur couvrent aussi les ressources absentes, liens sortants, préimages
modifiées et skills voisins à préserver.

Essai comportemental sur une famille Git jetable : le principal est `keep` et
le lié propre `cleanup_candidate`. L’ajout d’un fichier non suivi fait passer le
lié à `keep`. La lecture paginée conserve les deux identités ; une vue filtrée
à un candidat garde le total source de deux. Les longueurs et SHA-256 des artefacts
correspondent au reçu. Les manifestes de fichiers et métadonnées avant/après
restent identiques, hors atime exclu par l’oracle. Aucun dépôt réel n’a été scanné.

Routage BM25 local vérifié pour les deux clients : 12 scénarios positifs et
6 négatifs, skill éligible, seuil de qualité global satisfait. F1 de calibration
0,923 ; F1 d’évaluation croisée 0,649 ; F1 global 0,739. Ces scores ne constituent
pas une absence de faux positifs. Les 12 sondes de routage exécutées par client
passent : quatre demandes positives nouvelles en français/anglais et huit
négatives, dont les six du corpus. Les caches réels ont été reconstruits pour ce
checkout, après essai dans un cache temporaire. Aucun hook ou réglage global
n’a changé.

Validation locale : formatage, Clippy et compilation réussis ; 280 tests Rust
réussis, 2 ignorés. La suite Python a exécuté 260 cas : 254 réussis et 6 ignorés.
Les deux tests du parcours du skill et les 14 tests du générateur ont aussi été
exécutés séparément avec succès après les derniers ajustements. Les 6 tests
RETEX passent. Le contrôle documentaire couvre 28 documents, 306 liens locaux
et 33 blocs shell. Ces résultats ne prouvent pas une exécution de la CI distante.

La présence des projections et le routage sont observés. Le chargement et
l’invocation du nouveau skill dans une session native fraîche restent `UNKNOWN`.
L’essai CLI ci-dessus est un test des recettes, pas une évaluation d’un modèle
chargé automatiquement par le client.

## Choix des modèles du skill, 06/10/2026

La [politique de modèles](../harness/skills/wt-sweep/references/model-policy.md)
précise quatre profils, de l’extraction factuelle à la revue indépendante. Elle
vérifie les capacités du client avant de demander un modèle ou un niveau de
raisonnement, respecte les choix de l’utilisateur et distingue une recommandation,
une invocation acceptée et un réglage effectif confirmé. Les réglages persistants
et les modèles des rôles de développement restent inchangés.

Deux sous-agents au maximum peuvent lire des lots indépendants de rapports déjà
collectés, sous réserve des places disponibles. Le coordinateur garde la collecte,
les stashes partagés et la synthèse. Une preuve absente n’est pas un motif pour
augmenter le raisonnement. Les recommandations de profils n’ont pas fait l’objet
d’un benchmark de qualité ou de coût.

Une revue séparée a été exécutée avec `gpt-6.1-sol` et `high` demandés au lancement.
Le sous-agent a relu les règles et six scénarios : rapport simple, lots parallèles,
contrôle indisponible, budget Git épuisé, retour partiel ou verdict altéré, choix
explicite de l’utilisateur avec une place occupée. Aucun défaut actionnable n’a
été signalé. L’acceptation des arguments est observée ; cette revue ne mesure ni
le modèle effectif du fournisseur, ni le chargement natif du skill, ni sa qualité
comparée sur des rapports réels.

Après cet ajustement documentaire, les 14 tests du générateur et les 2 tests du
parcours CLI passent. Les 22 fichiers générés sont synchronisés. Les deux caches
BM25 ont été reconstruits : skill éligible, seuil global satisfait, 12 sondes sur
12 réussies par client. Le contrôle documentaire couvre 29 documents, 312 liens
locaux et 33 blocs shell. Les suites produit complètes
ne sont pas rejouées pour ce changement d’instructions ; leurs résultats au
commit précédent sont consignés ci-dessus.

## Historique U1-U4 et RETEX, 04-05/10/2026

Les sections suivantes conservent les résultats, l'état Git et les limites de
leurs dates d'observation. Leurs chiffres ne décrivent pas le triage courant.

Le 04/10/2026, le MVP macOS U1 à U4 est implémenté et vérifié sur des familles
Git jetables : inventaire, découverte, rapport local et import forge. La table
native des montages et la politique de refus de matérialisation sont raccordées.
La finition des 04 et 05/10 ajoute rapport descriptif, manifeste local explicite et dossiers
d'exécution ; ses preuves sont regroupées dans [finish-ledger.md](finish-ledger.md).
Le parcours global est testé avec volumes injectés ; aucun scan réel de la
machine n'a été exécuté. E1 reste `NON_DEMONTREE` et `apply` refuse l'exécution.

Autorité d'acceptation : [plan consolidé](plan.md). Répartition :
[implementation.md](implementation.md). Les observations locales ne valident
ni une CI distante, ni un scan de la machine, ni un retrait de dépôt.

## État initial et autorisation

Le socle recontrôlé avait 33 fichiers non suivis, une branche `main` sans HEAD,
5 tests Rust et 7 tests Python réussis. Formatage, Clippy et synchronisation des
12 projections passaient. Le checkout partagé a été conservé, avec un propriétaire
par fichier. À cette étape, aucun commit, push, fetch, prune, gc, nettoyage réel ou scan global
réel n'avait été exécuté. Les seules suppressions concernent les fixtures possédées
sous `/private/tmp`. Les sources historiques sont restées en lecture seule.

## Modèles et ownership observés

| Agent | Modèle et raisonnement | Fichiers et responsabilité |
|---|---|---|
| Intégrateur | Paramètres hérités de la session | Cargo, lib/model/main/output/review, reprise de discover et de ses tests pour la concurrence, tests CLI/modèle/sortie, documentation, CI et instructions canoniques |
| A, sweep-implementer | GPT-6.1 Sol, `high` | Collecteur Git et découverte initiale, `src/mounts.rs`, tests scan/mounts ; revue indépendante d'U4 et de residency |
| B, sweep-test-engineer | GPT-6.1 Sol, `high` | Oracles inventory/review/MVP/historical et helpers, `tests/review.rs` ; aucun code produit |
| C, sweep-implementer | GPT-6 Astra, `high` | Expérience E1, forge et reçus, `src/residency.rs` et ses tests ; revue indépendante d'U1/U3 et des montages |

L'API de collaboration expose et invoque ces rôles dans cette session. L'origine
exacte des définitions chargées et leur découverte automatique dans une nouvelle
session restent `UNKNOWN`. Aucun hook n'a été installé.

## U1 : inventaire exact d'une famille

`scan --repo` retourne principal, lié, détaché et inscription dont le dossier est
absent, avec l'ensemble exact attendu et des HEAD distincts. JSON préserve les
octets des chemins et les OID complets ; le tableau échappe les caractères de
contrôle. Le premier test réel échouait sur le refus du socle.

La revue a reproduit capture partageant un offset, écriture via TMPDIR hostile,
omission d'une administration cassée et substitution du parent de sortie.
Le collecteur contrôle maintenant les administrations, capture par sockets et
borne les appels Git. La sortie crée exclusivement un fichier, via descripteur
de répertoire et contrôle d'identité, hors du périmètre et des administrations.

L'oracle indépendant compare octets, modes, liens, mtime et ctime avant/après ;
l'atime est explicitement exclu. Les témoins fsmonitor/filtres/environnement
restent inactifs. EPERM/EACCES/EIO injectés ne deviennent pas une absence.
Un HEAD nul reste inconnu. La barrière avant U2 a passé 20 tests Rust, 21 tests
Python avec un cas sauté, E1, formatage, Clippy et synchronisation.

Le cas sauté demande un vrai nom non UTF-8 : ce système de fichiers refuse sa
création avec EILSEQ. Les octets non UTF-8 d'une inscription Git et du format
réversible sont testés. TCC réel, ACL et restauration des attributs restent inconnus.

La clôture du MVP a reproduit une mauvaise identification du principal avec
`git init --separate-git-dir`. Le collecteur vérifie désormais le toplevel,
l'administration commune et le pointeur `.git` avant de retenir le principal.
Un chemin d'administration sans backlink suffisant reste explicitement inconnu.
Les positifs principal séparé, linked et bare passent, y compris le rapport local.

## U2 : découverte et couverture

`scan --root` répétable et `scan --all` utilisent un parcours borné.
Les 14 tests de découverte couvrent `.git` fichier/dossier, bare à nom arbitraire,
administrations séparées, cachés, ignorés, dépôts imbriqués et sous-modules.
Ils comparent un ensemble exact non vide et un manifeste indépendant.

Alias, cycles, marqueurs cassés, accès refusés, limites et annulation sont
exposés. Les familles sont dédoublonnées par identité physique. Une observation
ultérieure contradictoire conserve ses faits et sa lacune. Les inscriptions hors
racines ont une origine explicite ; les liens sortants ne sont pas parcourus.
Volumes indisponibles et changements de montages sont éprouvés par injection.
Les contrôles dataless refusent les candidats concernés avant leur lecture Git.

Le fournisseur macOS lit maintenant tous les enregistrements `getfsstat` sans
filtrage silencieux, exige deux tables égales non saturées et conserve erreurs,
changements, source et type. Les 11 tests de montages couvrent fautes injectées
et lecture native de la seule table, sans parcourir les volumes. Un volume
disparu invalide la disponibilité de ses worktrees ; une table finale illisible
empêche de transformer cette incertitude en absence.

`--workers 1..8` règle les lectures parallèles de métadonnées, avec 1 par défaut.
Un témoin vérifie un chevauchement réel de quatre lectures et sa borne. Les
oracles comparent les mêmes faits à 1 et 4 workers, avec plusieurs budgets de
parcours ; l'agrégation reste déterministe. Les appels Git restent séquentiels.

La politique native de matérialisation dataless est désactivée et relue avant
les I/O, pour le processus et chaque thread. Son héritage par exec est testé,
afin de couvrir les chemins indirects de Git. Les includes, alternates et packs
résidents ont des cas positifs ; les FIFO déclenchent un refus ou un délai Git
borné, sans preuve d'intégration. Les [frontières natives](native-boundaries.md)
documentent l'ABI et les exceptions unsafe privées. Les deux frontières natives
initiales ont été relues mutuellement ; la sélection du cwd Git par descripteur
ajoutée en finition possède sa revue distincte.

Un véritable placeholder FileProvider, TCC et les montages non visibles restent
non éprouvés. Les observations ne sont pas un instantané atomique ; les appels
bloqués dans le noyau ne garantissent pas une interruption immédiate. `--all`
n'a jamais été lancé sur la machine de développement.

## U3 : relations locales et jugement pur

`plan --inventory` enrichit les faits, puis appelle un jugement sans I/O.
Les refs complètes `--target` désignent les cibles d'intégration ; les autres
restent intermédiaires. Aucune branche par défaut n'est devinée. Les relations
permises sont identité de commit, ascendance brute et identité d'arbre,
avec source, cible et OID explicites.

Les positifs produisent des observations non vides, y compris SHA-256.
Les cas S1 à S9, patch-id égal sur blocs répétés, grafts, replace, gitlinks masqués,
objets absents ou corrompus, inventaire périmé et commit tardif ne produisent
pas de fausse preuve. Les octets lus sont vérifiés contre leur OID, avec détection
de collisions SHA-1. Un marqueur shallow n'efface pas une ascendance brute
présente ; un parent manquant reste inconnu.

Les budgets bornent objets, octets, cibles et durée. Le schéma `wt-sweep.review/1`
refuse action/suggestion, reste déterministe à faits fixes et porte un digest
canonique. Un worktree déplacé depuis l'inventaire reste protégé contre une
sortie intérieure. Ce cas et un faux objet intermédiaire ont échoué avant correction.
Pour un périmètre global partiel, les ancêtres de sortie sont également inspectés
pour leurs marqueurs Git : une famille non encore découverte n'autorise pas
l'écriture dans son dossier. Le cas positif hors dépôt reste accepté.

Une MR ouverte précisément associée prime sur l'intégration, même dans un export
partiel. La tête source d'une MR fermée ou fusionnée n'est pas une cible.
Sans forge, son état reste inconnu ; l'âge seul ne crée pas d'activité.
`scan --local` collecte les fichiers et certains éléments d'administration avec
des limites explicites. Le rapport reprend ce manifeste en précisant que les
octets locaux ne sont pas revalidés ; ACL, attributs et reflogs partagés manquent.

## U4 : exports et reçus hors ligne

`forge commandes` affiche des modèles sans les exécuter. `forge ingerer` lit
exports GitLab et GitHub REST/GraphQL/CLI, pages concaténées ou groupées, et reçus
liés aux octets persistés. Un exécuteur de fixture produit réellement raw et reçu.

Les tests couvrent statut d'échec, saturation, troncature, page manquante,
curseur répété, versions contradictoires, projet homonyme/fork, remplacement
du raw et falsification de l'artefact normalisé. Les faits positifs partiels
restent disponibles. La revue indépendante a reproduit deux défauts : choix
silencieux entre champs contradictoires et omission d'une edge GraphQL sans node.
Ces cas ont échoué avant correction. Les contradictions invalident l'association
précise ; les edges incomplètes restent comptées et signalées. Un cas positif
d'alternatives concordantes empêche un refus systématique.

Pagination complète, cohérence temporelle et absence prouvée restent distinctes.
Les adaptateurs ne certifient aucune absence. Un reçu cohérent lie des octets
à des métadonnées déclarées ; il n'authentifie pas l'exécuteur. Le rapport relit
les preuves originales avant association. L'état actuel du serveur reste inconnu.

Un import historique autorisé, en lecture seule, a conservé 430 demandes issues
de cinq pages de tailles 100/100/100/100/30. Son test est ignoré par défaut et exige
un chemin explicite ; seules les fixtures synthétiques entrent dans le dépôt.

Quatre replays Python supplémentaires vérifient cet ensemble et les cas !91,
!92 de stats-api, puis la MR !383 associée au worktree `lead-app-mr43`. Le suffixe
du worktree n'est pas l'IID de sa MR. Les 104 entrées des deux pages stats et les
430 entrées lead-app sont conservées. Les trois SHA de fusion manquants restent
`null`, sans inventer de preuve depuis l'état de la MR ou sa tête source.
Le manifeste indépendant confirme la conservation des sources, hors atime.
Sans reçu d'exécuteur, la provenance reste partielle ; la disponibilité actuelle
des objets et l'intégration historique restent `UNKNOWN`. Aucun dépôt réel n'a
été consulté pour compléter ces absences. Ces replays exigent un chemin autorisé
via `WT_SWEEP_HISTORICAL_FORGE_DIR` et sont sautés par défaut.

## E1 et suite

La reprise des [retours msds et Aristote](retex-sessions.md) ajoute cinq tests
aux témoins E1 : ignorés supprimés sans force, non-suivis refusés, empreinte de
statut insensible au contenu, index distinct perdu par un patch depuis HEAD et
observation native d'un écrivain au cwd extérieur. Les six tests passent ;
aucune fonctionnalité de retrait n'est ajoutée. Les huit pistes sont évaluées
et trois lots de lecture seule sont raccordés pendant la finition.

La [décision E1](decisions/exclusion-et-conservation.md) documente les témoins
réels : descripteur extérieur après renommage/verrou coopératif, écriture à
taille/mtime identiques et commit détaché tardif. Le test valide des contre-exemples,
sans prouver l'exclusion. ACL/attributs/types spéciaux, restauration indépendante
et durabilité restent `UNKNOWN`. U5 à U7 ne sont pas implémentées.
La [sonde de capacités natives](decisions/apply-readiness.md) constate EPERM
sur le lease d'écriture, ENOTSUP sur revoke d'un fichier régulier, et l'écriture
toujours possible via le descripteur préexistant. Aucun privilège ou montage
n'a été modifié. Il faut éprouver une frontière d'autorité distincte avant U5.

Le mandat du MVP U1 à U4 est terminé sur les fixtures et frontières décrites.
U5 exige séparément E1 concluante et la comparaison des alternatives aux mêmes
fixtures. Aucun succès des tests actuels ne lève cette condition.

## Vérification intégrée avant les lots du RETEX

Vérification finale du 04/10/2026, après raccordement des frontières natives,
concurrence, corrections des administrations séparées et des sorties globales :

| Contrôle exécuté | Résultat observé |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --all-targets --locked --offline -- -D warnings` | PASS |
| `cargo test --locked --offline` | 80 passés ; 2 ignorés : historique explicite et entrée de sous-processus appelée par son test parent |
| Test historique, chemin explicite et `--ignored` | 1 passé, 430 demandes conservées |
| Replays Python, chemin historique explicite | 4 passés, !91 / !92 / !383 et 430 demandes |
| `cargo build --locked --offline` | PASS |
| `cargo build --release --locked --offline` | PASS ; binaire optimisé disponible localement |
| Parcours sur le binaire optimisé | 4 passés : ascendance, sortie du rapport, workers, administration séparée |
| Python 3.11+, découverte `tests/test_*.py` | 68 recensés : 63 passés, 4 historiques sautés par défaut mais validés séparément, 1 saut EILSEQ décrit plus haut |
| Python, découverte `experiments/exclusion/test_*.py` | 6 passés, dont 5 issus des retours de session ; exclusion toujours NON_DEMONTREE |
| `scripts/sync-ai.py --check` | 12 fichiers synchronisés ; activation native UNKNOWN |

La régression indépendante des identités forge contradictoires passe pour ses
trois sous-cas : projet, HEAD, et les deux ensemble. Les faux objets intermédiaires,
blobs absents, positifs SHA-256 et protections des worktrees déplacés passent.
La CI distante n'a pas été exécutée.

La revue finale indépendante de C n'a trouvé aucun défaut bloquant concret dans
les raccordements de concurrence, gardes natives, changements de volumes,
contrôles de sortie globale et rescans via le principal. Elle ne remplace pas
les tests exécutés et ne lève aucune des limites FileProvider, TCC ou E1.

État Git à cette étape : 64 fichiers non suivis, sur `main` sans premier commit, aucun
autre statut. Les changements couvrent les dix modules produit, les fixtures
et tests, Cargo, la CI et la documentation. La source canonique des instructions
renvoie désormais à ce registre ; AGENTS/CLAUDE et le manifeste ont été régénérés.
Les définitions des agents restent inchangées. Le [changelog](../CHANGELOG.md)
recense les changements. À cette étape : aucun commit, push ou publication.

La clôture du 05/10, après les trois lots du RETEX et leurs corrections de revue,
est consignée dans [finish-ledger.md](finish-ledger.md) : 92 tests Rust et 78 tests
Python passent, ainsi que 6 témoins E1/RETEX et 5 parcours sur le binaire optimisé.
E1, U5 à U7 et la validation distante gardent les limites décrites plus haut.

Git interdit les transports avec une liste vide `GIT_ALLOW_PROTOCOL`, en plus
du refus des téléchargements implicites. Le test réel vérifie qu'une configuration
locale autorisant le protocole ne réactive pas son exécuteur. Référence primaire :
[documentation Git des variables de transport](https://git-scm.com/docs/git/2.53.0.html).


## Audit lisible : intégration du 07/10/2026

Le lot courant implémente `--html` sur `scan`, `review` et `plan`, le rendu
hors ligne par famille et le reçu de publication du HTML. Les étapes de
progression affichent des observations provisoires d’histoire et d’état local.
Le correctif d’index `intent-to-add` conserve la distinction index/disque.
Ce paragraphe décrit le checkout en cours ; le SHA livré et la validation
complète sont consignés à la fin du lot, séparément des passages ciblés.

Contrôles ciblés observés : 10 tests de rendu HTML, 9 de publication, 5 parcours
CLI HTML (dont multi-racines avec doublon, worktrees extérieurs enregistrés et
racine inaccessible), 8 tests de terminal/PTY. Les 2 régressions du formatage
progressif ont échoué avant implémentation puis réussi. Les reçus lient les
longueurs et hashes ; les oracles de fixtures ne constatent pas de mutation.

Chromium a ouvert un rapport synthétique de 2 familles et 4 worktrees à
1920, 1280, 390 et 320 px, avec et sans JavaScript. Aucun appel réseau, erreur
JavaScript ou débordement horizontal observé. Recherche, filtre de verdict,
tri, thème et détails au clavier vérifiés ; rendu contrôlé à zoom CSS 200 %.
Ce dernier contrôle ne prouve pas le comportement de tous les navigateurs au
zoom natif. Captures desktop/mobile relues visuellement. Une revue indépendante
a trouvé l’omission des lacunes de manifestes `scan --local` ; corrigée, testée
et relue. Les mesures de performance et la recette réelle ont leurs preuves
séparées ; ces tests n’établissent pas un inventaire actuel de la machine.


Le skill reste de portée dépôt : source `harness/skills/wt-sweep`, projections
`.claude/skills/wt-sweep` et `.agents/skills/wt-sweep`. `sync-ai.py --check`
valide 22 fichiers. Les caches BM25 de ce checkout ont été reconstruits pour
Claude et Codex, sans changement des hooks ni des réglages globaux. Chaque client
sélectionne le skill sur les 14 sondes positives et l’évite sur les 6 négatives
exécutées. Il est éligible et le seuil global passe ; F1 de calibration 0,966,
F1 de l’évaluation croisée 0,683. Ces sondes ne prouvent pas une absence générale
de faux positifs. L’invocation native dans une nouvelle session reste inconnue.

Le prompt « reprendre un rapport sans relancer l’analyse » a été rejoué par
un sous-agent sur le rapport synthétique de quatre lignes : trois `keep`, un
candidat. Le lecteur a vérifié les tailles et SHA-256 des trois artefacts
liés au reçu, cité les motifs et conditions de chaque ligne, puis proposé de
préserver la note locale et de confirmer l’usage du candidat. Aucun scan ni
retrait exécuté ; artefacts inchangés. Ce cas possède des tailles complètes et
aucun filtre : les cas partiels et filtrés relèvent des tests séparés. Ce rejeu
contrôlé ne démontre pas l’activation native du skill dans une nouvelle session.

### Vérifications locales du lot audit

Sur le code Rust et les assets HTML finaux, `cargo fmt --all -- --check` et
Clippy tous targets passent. La suite Rust complète compte 361 succès et
3 ignorés : benchmark volontaire, entrée enfant exécutée par son test parent et
import historique nécessitant des données explicitement choisies. La suite
Python complète compte 291 cas, 285 succès et 6 ignorés : quatre imports
historiques non demandés et deux créations de chemins refusées par le système
de fichiers (EILSEQ). Les cas d’identités en octets restent testés séparément.
Les six témoins d’exclusion passent ; ils démontrent les contre-exemples, pas
l’exclusion des écrivains.

Le SHA-256 du binaire debug est resté
`14183414de93e18921550ef01ed1e990ebb1438e828dba76f395122b41bb3a59`
avant et après la suite Python. La release des essais navigateur et des douze
scénarios comparatifs porte l’empreinte
`a622edf51eb32ef8fe58991703f9b404bbc67d1526bddaec9f4950206d6f26bf`.
La capsule de sources construite conserve Cargo, tous les fichiers Rust et les
assets CSS/JS. Les tests du protocole de benchmark ajoutés ensuite sont
consignés séparément. La CI du commit livré reste une preuve distincte.


Le [benchmark de collecte du 07/10](research/collection-benchmark-2026-10-07.md)
conserve huit échantillons propres comparables et deux échantillons avec fichiers
ignorés. Un couple standard à 100 worktrees reste explicitement non comparable.
Les six tests du protocole ajoutés après la suite Python complète passent
séparément ; ils vérifient notamment l’identité des preuves, la persistance des
mesures refusées et l’arrêt du groupe de processus possédé. Le test de signal
observe le vrai appel système mais ne constitue pas un oracle de disparition de
chaque descendant. Les deux tests du banc de comparaison des outils passent.
