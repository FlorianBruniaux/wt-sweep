# Recette locale de la comparaison de contenu

Le 13 octobre 2026, Codex a exécuté le
[plan v2](../superpowers/plans/2026-10-13-local-content-comparison-v2.md) dans le
checkout de `wt-sweep`, à la demande de Florian. Livraison locale, sans commit,
push, release ni nettoyage réel. Les contrôles requis sont terminés le
14 octobre 2026 sur macOS.

## Fonction livrée

`review` et `plan` acceptent `--compare-local` pour un rapport enregistré.
`--compare-reference` ajoute une copie explicitement choisie dans la même famille
Git physique. Le rapport répond à « Que reste-t-il à récupérer ? » avec les
chemins identiques dans une cible Git, identiques dans la copie, différents ou
non comparés. Le menu et la recherche de sections donnent accès à ce bilan.

Le sidecar `local-comparison.json` conserve les deux côtés de chaque comparaison,
les OID complets, empreintes, types, modes, témoins physiques, références et
limites. Sa validation recalcule résultats, comptes et couverture à partir des
preuves enregistrées et des comptes source. Les couches index/disque et les
stages de conflit restent distincts. Un fichier ignoré ou une configuration de
version de Node différent conserve son observation.

Une égalité de contenu ne modifie pas `review/2`, `triage/1`, C1-C5 ni un verdict
de conservation. Une comparaison incomplète conserve ses inconnues ; une
référence inaccessible ne masque pas une égalité établie avec une autre cible.

## Contrôles observés

| Contrôle | Résultat après corrections |
| --- | --- |
| Formatage Rust | Passe |
| Clippy, tous les targets, avertissements refusés | Passe après corrections |
| Tests Rust | Suite complète : 467 tests passent, dont 11 tests d'intégration ciblés et la régression de dérive |
| Compilation hors ligne | Passe après corrections |
| Suite Python du projet | 342 tests exécutés : 336 passent, 6 sont ignorés |
| Expériences d'exclusion | Les 6 tests passent |
| Synchronisation du harness | Passe après corrections : 22 fichiers synchronisés ; activation native inconnue |
| Éligibilité BM25 Codex et Claude | Passe dans deux caches de test ; sonde positive vers `wt-sweep`, sonde négative vers `msg` |

Les tests utilisent des dépôts Git réels, synthétiques et jetables. Le corpus
couvre SHA-1/SHA-256, un octet différent, mode seul, suppression contre fichier
vide, index différent du disque, stages de conflit, noms non UTF-8, fichiers
ignorés, symlinks, Gitlinks, objets corrompus, preuve positive avec cible manquante,
caps, dérive, refus de publication dans la référence et défaut de synchronisation
avant le reçu. Les CLI tests comparent aussi les manifestes source avant/après.

Les six tests ignorés ne valident aucun cas : quatre replays historiques exigent
un répertoire de forge explicitement autorisé, absent de cette exécution ; deux
tests de création de chemins non UTF-8 sont refusés par le système de fichiers
avec `EILSEQ`. Les chemins Git/index en octets bruts restent testés. Les 26 tests
CLI de la nouvelle comparaison passent dans la suite complète.

Commandes observées, depuis la racine du dépôt :

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked --offline -- -D warnings
cargo test --locked --offline
cargo build --locked --offline
python3.11 -B -m unittest discover -s tests -p 'test_*.py' -v
python3.11 -B -m unittest discover -s experiments/exclusion -p 'test_*.py' -v
python3.11 -B scripts/sync-ai.py --check
```

Les commandes Cargo passent par `rtk proxy` ; Python utilise le chemin absolu de
la version 3.11.15 installée. La suite Python du projet exige la permission
d'exécution décrite ci-dessous pour ses contrôles de processus enfants.

## Revue indépendante et corrections

La [revue indépendante](../superpowers/plans/2026-10-13-local-content-comparison-final-review.md)
a demandé trois corrections Important et signalé un défaut Minor. L'intégrateur
a reproduit chacun avant correction :

| Finding | Échec reproduit | Correction et vérification |
| --- | --- | --- |
| F1 | `/linked/.` produisait une auto-comparaison ; une revue de toute la famille collectait aussi la référence elle-même. | Refus par identité physique ; ligne de référence conservée comme non demandée pour la famille entière. Tests CLI : passent. |
| F2 | Une réécriture en place de `.git` vers une autre famille laissait la référence stable. | Capture et revalidation du marqueur, des pointeurs, administrations et famille sous le budget restant. Test réel avec injection de mutation : passe. |
| F3 | `cache/config` dans un dépôt bare imbriqué pouvait être déclaré identique à une copie ordinaire. | Refus de traverser la signature bornée `HEAD`/`objects`/`refs`. Test CLI : passe. |
| F4 | Une suppression disque avait `parent: null` dans le sidecar. | Conservation du témoin d'absence final ; le validateur refuse son retrait même après recalcul du digest. Tests Rust et CLI : passent. |

Un seul passage de correction a suivi cette revue. Il n'y a pas de seconde
approbation indépendante. Le reviewer a refusé de juger les suites encore en
cours, le rendu navigateur, les performances réelles, la concurrence reproduite
et l'activation native. Les régressions de concurrence ci-dessus ont ensuite
été exécutées par l'intégrateur.

## Arbitrages de l'implémentation

- Exécution dans le checkout demandé, déjà modifié : préimages de 328 fichiers
  avant édition. Les 298 fichiers hors périmètre restent identiques, octets et
  modes ; 35 fichiers sont dans le périmètre final. La restauration
  automatique du checkout partagé reste indisponible sans exclusion des writers.
- Checkpoint et rollback démontrés seulement sur fixtures possédées. Un
  postimage concurrent modifié refuse la restauration ; le fichier sentinelle
  extérieur reste intact.
- Le lecteur partagé a intégré références Git multiples et copie explicite
  dès le premier lot vertical. Les gates de chaque lot restent testés, mais le
  premier diff était plus large que le découpage initial.
- Python 3.11.15 est appelé directement, car le shim n'a pas de version active.
  Aucun changement de configuration globale.
- Le générateur a rencontré une restriction sur `.agents`. Une exécution avec
  permission limitée à la génération a terminé les projections. Les fichiers
  canoniques et les deux projections sont synchronisés ; activation native
  inconnue.
- Les caches BM25 de test respectent la frontière sans écriture globale.
  Ils vérifient l'éligibilité dans le corpus actuel, sans activer de hook.
- La première suite globale, sur le snapshot examiné, a été interrompue après
  les findings pour être remplacée par une suite complète sur la version
  corrigée. Ses contrôles Rust avaient passé ; la portion Python interrompue
  ne vaut pas validation complète.
- Dans Codex, les préflights d'interruption Python ont échoué avant de lancer
  leur CLI : `/bin/ps` est refusé, `pgrep` sort avec le code 3 et ne peut obtenir
  la liste des processus. Les binaires existent et les commandes nues confirment
  le refus. Les réglages `excludedCommands` de Claude ne s'appliquent pas à ce
  runtime. Le test ciblé passe avec une permission d'exécution adaptée, puis la
  suite Python entière est relancée ainsi. Aucun réglage global n'est modifié.

## Limites de preuve et livraison

L'ouverture automatisée du rapport `file://` a été refusée par la politique du
navigateur. Aucun serveur ni autre navigateur n'a servi de contournement. HTML,
liens et comportements testables sont contrôlés par les tests disponibles ; le
rendu visuel navigateur reste non vérifié.

Aucun scan global, fetch, prune, gc ou effacement de worktree réel n'a été
exécuté. Le rapport de démonstration utilise une famille synthétique. Les anciens
rapports personnels ne gagnent pas de preuves nouvelles sans revue fraîche.

La démonstration corrigée a une comparaison complète : un chemin identique dans
la cible Git, un dans la copie locale et un différent. Les tailles et SHA-256
de ses cinq artefacts correspondent au reçu ; son manifeste source est identique
avant/après. Le parcours séparé `scan` puis `plan` passe également et conserve
l'inventaire d'entrée octet pour octet.

Les journaux locaux et préimages sont conservés sous
`/private/tmp/wt-sweep-local-comparison-checkpoint`. Ils ne constituent pas une
sauvegarde durable. La CI distante, Linux, les performances réelles et la
publication restent non vérifiés.
