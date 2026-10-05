# Worktree Sweep (`wt-sweep`)

CLI Rust pour inventorier les dépôts et worktrees Git puis produire un rapport
local explicable. Le MVP en lecture seule est implémenté pour macOS : découverte,
relations Git et import forge hors ligne. Le nom reste provisoire ; `apply`
reste indisponible.

## Utilisation

```sh
cargo build --locked --offline
# Une famille explicitement choisie, JSON réversible ou tableau :
target/debug/wt-sweep scan --repo /chemin/clone --format json
target/debug/wt-sweep scan --repo /chemin/clone --out /autre/inventory.json
# Inclure les fichiers locaux et l'index, dans une exécution isolée :
target/debug/wt-sweep scan --repo /chemin/clone --local --run-dir /autre/runs
# Racines ciblées, sans exclusions implicites de dossiers cachés ou ignorés :
target/debug/wt-sweep scan --root /racine/a --root /racine/b --workers 4 --format json
# Rapport local, cible d'intégration choisie explicitement :
target/debug/wt-sweep plan --inventory /autre/inventory.json \
  --target refs/heads/main --out /autre/review.json
target/debug/wt-sweep plan --inventory /autre/inventory.json \
  --target refs/heads/main --summary --run-dir /autre/runs
# Imports hors ligne, aucune commande distante exécutée :
target/debug/wt-sweep forge commandes
target/debug/wt-sweep forge ingerer --raw /autre/raw.json \
  --receipt /autre/receipt.json --out /autre/forge.json
target/debug/wt-sweep plan --inventory /autre/inventory.json \
  --forge /autre/forge.json --target refs/heads/main --format json
```

`--repo`, `--root` et `--all` sont mutuellement exclusifs. Aucun scan n'est
implicite. `--all` part des montages retournés par le noyau et expose ses lacunes ;
il n'a jamais été exécuté sur la machine de développement. Les tests injectent
leurs racines et volumes. `--workers 1..8` borne les lectures parallèles de
métadonnées, avec une valeur par défaut de 1. L'ordre d'agrégation et les
observations restent identiques sur une fixture stable. Les appels Git restent
séquentiels et bornés.

Avant les lectures, le processus désactive la matérialisation des fichiers
dataless, vérifie cette politique et la transmet à Git. Un échec provoque un
refus explicite. Cette protection couvre aussi les lectures indirectes de Git ;
elle ne constitue pas un instantané du disque. Les autres systèmes refusent les
opérations exigeant cette protection. Voir les [frontières natives](docs/native-boundaries.md).

`--out` crée exclusivement un nouveau JSON, hors des familles et du périmètre
inspectés, puis affiche un tableau. Il refuse les sorties existantes et les
ancêtres symboliques. La publication et son nettoyage utilisent un descripteur
de répertoire, avec contrôle d'identité ; cela ne démontre pas l'exclusion de
tous les écrivains ou l'immobilité globale des noms. Pour un inventaire global,
les marqueurs Git des ancêtres sont aussi vérifiés, même si le parcours partiel
n'a pas encore découvert la famille concernée.

`--run-dir BASE` demande une base existante hors des cibles et crée un dossier
unique contenant le JSON, `report.txt` et `run.json`. Ce reçu donne les empreintes
des deux artefacts, les dates de publication et un état `complete` ou `partial`.
Les dates de collecte sont dans les artefacts. Deux processus créent des dossiers
distincts ; aucun fichier précédent n'est remplacé. `--run-dir` et `--out` sont
incompatibles. Un dossier sans reçu valide est incomplet, même si des fichiers
s'y trouvent. Ces synchronisations ne constituent pas le contrat de sauvegarde U5.

Le rapport décrit sujet du commit, branche, HEAD complet, MR associées, preuves
et inconnues. `--summary` produit cinq lignes et indique le détail conservé quand
`--run-dir` est utilisé. Les sujets sont bornés à 240 octets, les contrôles sont
échappés, les titres n'ont aucune autorité sur le classement. `--summary` et
`--format json` sont incompatibles.

`scan --local` collecte types, modes, propriétaire, identités, dates, liens et
SHA-256 des fichiers résidents, y compris cachés et ignorés. Il conserve l'index
brut, l'index partagé identifiable et le reflog HEAD propre au worktree. Les index
partagés restent sans interprétation sémantique : la lecture Git testée modifie
leurs métadonnées, donc l'outil expose cette lacune. Le parseur traite les versions
2 et 3 ; version 4 et extensions obligatoires non reconnues restent inconnues.
Les règles d'ignorés passent par une commande Git qui ne lit pas l'index.
ACL, attributs étendus,
reflogs partagés et sauvegarde des objets restent hors de ce manifeste.

Les limites par worktree sont 20 000 entrées, 64 Mio par fichier, 256 Mio lus,
8 Mio de manifeste compact, 256 niveaux et 60 secondes. L'ensemble d'un scan
local est limité à 25 000 entrées, 256 Mio de contenu publié, 24 Mio de manifestes
compacts et 120 secondes. Chaque dépassement laisse une lacune explicite ; les
appels bloqués dans le noyau ne garantissent pas une interruption immédiate.
Un JSON publié ne peut dépasser 32 Mio ; réduire la portée si cette limite est
atteinte. Le manifeste décrit un intervalle, sans instantané atomique. `plan`
le transporte et signale que les contenus locaux n'ont pas été relus.

## Contrats et limites

- `wt-sweep.inventory/1` conserve chemins en octets hexadécimaux, OID complets,
  identités, états de chemins, inscriptions Git, lacunes et intervalle observé.
  Les erreurs d'accès ne deviennent pas des absences. Les domaines non
  inventoriés restent explicites ; `local_inventories` est renseigné uniquement
  quand `--local` est demandé.
- `wt-sweep.review/1` contient des observations, risques et inconnues, aucune
  action. Les relations permises sont identité de commit, ascendance par lecture
  des objets commit et identité d'arbre. Les octets des commits, arbres et blobs
  lus sont vérifiés contre leur OID ; un objet absent ou corrompu reste inconnu.
  Les gitlinks appartiennent aux arbres.
  Une cible doit être nommée par `--target` pour porter un classement
  `integration_observed` ; les autres refs restent des cibles intermédiaires.
  Patch-id, simulation de fusion, âge et fermeture d'une MR ne sont pas des preuves.
- `wt-sweep.forge/1` conserve faits positifs et provenance des exports, même
  partiels. Les reçus doivent venir de l'exécuteur ayant persisté les octets.
  Pagination complète et vue cohérente sont distinctes ; aucune absence de MR
  n'est certifiée par les adaptateurs actuels. Une observation historique ne
  certifie pas l'état actuel du serveur.

Le [registre d'implémentation](docs/progress.md) distingue les critères éprouvés
et ceux encore ouverts. [E1](docs/decisions/apply-readiness.md) reste
`NON_DEMONTREE` : le produit ne fournit ni sauvegarde admissible ni retrait.
Les [retours des sessions de ménage](docs/retex-sessions.md) ont fourni cinq
témoins et les trois lots de rapport, manifeste et exécution isolée décrits ici.
Les montages non visibles, les permissions TCC réelles, le comportement d'un
véritable placeholder FileProvider et la cohérence temporelle ne sont pas
certifiés. Deux tables de montages identiques restent des observations successives,
pas une preuve d'immobilité de la machine.

Avant un futur moteur de nettoyage, comparer agentrinse et les autres candidats
aux mêmes fixtures. Les [références](docs/references.md) ne constituent pas une
validation comportementale de ces alternatives.

## Développement

macOS, Rust 1.95.0 et Python 3.11 ou ultérieur pour les tests. `Cargo.lock` est versionné ; les
bibliothèques utilisées sont serde/serde_json, rustix pour les publications par
descripteur, libc pour les frontières macOS, SHA-256 et SHA-1 avec détection de
collisions pour les objets Git.
Après une récupération explicite des dépendances,
les vérifications fonctionnent hors ligne :

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked --offline -- -D warnings
cargo test --locked --offline
cargo build --locked --offline
python3 -B -m unittest discover -s tests -p 'test_*.py' -v
python3 -B -m unittest discover -s experiments/exclusion -p 'test_*.py' -v
python3 -B scripts/sync-ai.py --check
```

Sur macOS, choisir un Python récent si `python3` ne fournit pas `tomllib`.
Les tests ne nettoient que leurs propres fixtures temporaires. La suite Python
produit utilise `target/debug/wt-sweep`, à reconstruire avant de la lancer.

[Plan](docs/plan.md), [ownership](docs/implementation.md),
[harness IA](docs/harness.md), [changelog](CHANGELOG.md).
Aucun service, compte forge ou modèle n'est nécessaire à l'exécution du produit.
La publication Cargo est désactivée ; licence et dépôt distant restent à choisir.
`cargo build --release --locked --offline` produit le binaire optimisé dans
`target/release/wt-sweep`, sans installation ni publication.
