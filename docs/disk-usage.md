# Espace disque des worktrees

[Index documentaire](README.md) · [Référence CLI](cli.md)

`scan --disk-usage` et `review --disk-usage` mesurent les dossiers des worktrees
découverts et le dossier Git commun de chaque dépôt. Le rapport classe les mesures
par taille allouée décroissante. Le collecteur Rust calcule les tailles et le
rendu les inscrit dans les rapports à partir des données capturées, sans LLM.
Ces chiffres ne participent pas à la décision de nettoyage.

## Lancer une mesure

Depuis le checkout de Worktree Sweep, sur macOS :

```sh
(
  set -eu
  cargo build --release --locked --offline
  report_dir="$(mktemp -d /private/tmp/wt-sweep-disk.XXXXXX)"
  ./target/release/wt-sweep scan \
    --root "$HOME/Sites" \
    --root "$HOME/.codex/worktrees" \
    --disk-usage \
    --disk-budget thorough \
    --html \
    --run-dir "$report_dir"
  open "$report_dir"/run-*/report.html
)
```

Les `XXXXXX` sont remplacés automatiquement par `mktemp`, pour créer un dossier
neuf. Pour cibler un dépôt, remplacer les deux `--root` par `--repo` suivi de son
chemin absolu. Pour les volumes montés visibles, les remplacer par `--all`.
La découverte conserve sa limite de 60 secondes : `--all` ne garantit pas que
chaque worktree de la machine sera trouvé. Les enregistrements Git peuvent
ajouter des worktrees situés hors des racines parcourues.

## Mesurer et analyser dans la même commande

Pour un dépôt et sa cible d’intégration, depuis le checkout de Worktree Sweep :

```sh
(
  set -eu
  cargo build --release --locked --offline
  report_dir="$(mktemp -d /private/tmp/wt-sweep-review.XXXXXX)"
  ./target/release/wt-sweep review \
    --repo "$HOME/Sites/MethodeAristote/app" \
    --target refs/heads/develop \
    --disk-usage \
    --disk-budget thorough \
    --group-by directory \
    --summary \
    --html \
    --run-dir "$report_dir"
  open "$report_dir"/run-*/report.html
)
```

La section `DISK USAGE` apparaît dans le récapitulatif console et dans le rapport
final `report.txt`. Le détail liste les tailles par dossier ; le récapitulatif
affiche le total et la couverture de la mesure. Les octets exacts sont également
dans `inventory.json` et `review.json`. Une mesure partielle reste marquée
`INCOMPLETE` à côté du volume affiché. La progression compte les dossiers visités
sans afficher tous leurs chemins ; `--verbose` rétablit ces lignes et les
métriques détaillées. Les intervalles de mesure restent dans le rapport complet.

`--disk-usage` reste explicite, car parcourir les métadonnées peut être coûteux.
Sans cette option, `review` indique que les tailles n’ont pas été mesurées.
`plan --inventory ...` reprend les mesures disponibles dans cet inventaire,
avec leur intervalle d’origine, sans les recalculer. Un inventaire ancien sans
tailles reste lisible. Les mêmes observations donnent le même rapport de tailles.

Le budget disque est indépendant de `--budget`, qui concerne les vérifications
Git et locales. Le comptage disque couvre **toutes les racines inventoriées** :
`--worktree` et les filtres de présentation ne le réduisent pas. Les totaux sont
étiquetés en conséquence, même si une seule ligne de revue est sélectionnée.

Avec `--html --run-dir`, le [rapport HTML par famille](html-report-plan.md)
affiche les mêmes mesures capturées. La recherche et les filtres du navigateur
changent les lignes visibles, sans recalculer les totaux source. L’absence de
mesure et les lacunes restent affichées ; `run.json` lie aussi `report.html`.
`report.txt` reste disponible dans le même dossier.

## Lire le bilan

| Valeur | Ce qu’elle mesure |
|---|---|
| `allocated` | Nombre de blocs déclaré par le système de fichiers, multiplié par 512 octets. Inclut les blocs des répertoires. |
| `logical` | Longueur des fichiers réguliers et des liens symboliques. Les fichiers creux peuvent avoir une longueur très supérieure à leur allocation. |
| `Worktree` | Dossier de travail, avec fichiers cachés, ignorés, `node_modules` et caches présents sous ce dossier. |
| `Shared Git` | Administration commune, objets et métadonnées Git. Comptée séparément une fois par chemin découvert. |
| Total dédupliqué | Chaque couple périphérique/inode observé est compté une seule fois, tous dossiers confondus. |
| `complete` | Parcours de cette racine terminé sans lacune détectée. Ne certifie pas une photographie instantanée des fichiers. |
| `partial` | Une partie seulement a été mesurée ; la valeur affichée est le cumul des observations. |
| `not measured` | Aucune taille disponible. Ce n’est pas une taille nulle. |

Un autre worktree enregistré imbriqué dans un dossier est mesuré dans sa propre
ligne. Les liens physiques sont dédupliqués dans une ligne et dans le total.
Les lignes peuvent toutefois partager des inodes entre elles : leur somme peut
être supérieure au total. Les cibles des liens symboliques ne sont pas suivies.
Un dépôt nu apparaît seulement dans les administrations Git.

**La taille allouée n’est pas une promesse d’espace récupérable.** Des clones APFS,
instantanés ou extents partagés peuvent changer ce qu’une suppression libérerait.
Le collecteur ne mesure pas ces relations. Voir la [description des clones APFS par Apple](https://developer.apple.com/documentation/foundation/about-apple-file-system). Il ne parcourt pas non plus les
alternates Git ou les dépendances situées hors des dossiers mesurés.

## Coût et limites

Le comptage des tailles est séquentiel ; `--workers` concerne la découverte.
La mesure lit les métadonnées, sans lire ni hacher le contenu des fichiers et
sans lancer Git pendant le comptage. Elle utilise des descripteurs de répertoires
et refuse de suivre un lien symbolique pendant la descente. Elle vérifie
l’identité de chaque racine découverte et conserve les erreurs ou changements
de répertoires observés. Les fichiers peuvent néanmoins changer pendant le
parcours ; `temporal_consistency` reste `not_guaranteed`.

Les plafonds par défaut sont de 500 000 entrées et 30 secondes par racine,
2 000 000 entrées et 600 secondes pour l’ensemble, avec une profondeur de 128.
`--disk-budget thorough` porte ces plafonds à 1 000 000 entrées et 120 secondes
par racine, 10 000 000 entrées et 1 800 secondes au total. La profondeur reste 128.
Le profil étendu est explicite et peut consommer davantage de temps et de mémoire.
Les délais sont vérifiés entre opérations ; un appel système bloqué peut les
dépasser. Les franchissements de système de fichiers et les répertoires dataless
ne sont pas parcourus et créent une lacune. Un fichier dataless peut contribuer
par ses métadonnées, sans téléchargement de son contenu.

Chaque racine conserve au plus huit exemples de lacunes et un compteur des
exemples supplémentaires. Une mesure incomplète rend le reçu du scan ou de la revue partiel,
sans modifier la couverture de découverte. Les totaux concernent les racines
trouvées, même si la découverte en a manqué d’autres.

## JSON et validation

`inventory.json` et `review.json` reçoivent un champ optionnel `disk_usage`
(`source.disk_usage` dans une vue filtrée). Il contient les mesures,
les tailles exactes en octets, le total dédupliqué, les plafonds appliqués,
l’intervalle et la durée. Sans l’option, ce champ reste absent. Les rapports
antérieurs restent lisibles. La publication conserve ce bloc entier ou refuse
si les limites de sortie ne permettent pas de le publier ; elle ne réduit pas
silencieusement les totaux aux lignes d’inventaire conservées.

Les chiffres de taille ne participent pas aux verdicts de `plan` ou `review`.
Un petit worktree peut contenir un travail unique ; un gros worktree peut encore
être utilisé. Choisir une cible Git explicite pour la revue, comme dans le
[tutoriel](tutorial.fr.md). La mesure disque précède les vérifications de cette
revue ; son intervalle reste affiché séparément et les fichiers peuvent évoluer
entre les deux.

Tests du collecteur sur des dépôts temporaires :

```sh
cargo test --locked --offline --test disk_usage
```
