# Résultats d'implémentation

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
