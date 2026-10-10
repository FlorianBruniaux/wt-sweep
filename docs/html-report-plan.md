# Rapport HTML par projet

[Index documentaire](README.md) · [Référence CLI](cli.md) · [Tutoriel](tutorial.fr.md)

`--html` produit `report.html` avec `scan`, `review` et `plan`. Le fichier est
autonome : styles et script sont embarqués, sans ressource réseau ni dépendance
à un LLM. Il s’ouvre directement dans le navigateur. Le sommaire et les preuves
restent lisibles sans JavaScript.

## Générer et ouvrir

Depuis le checkout compilé, renseignez le repo et sa cible confirmée :

```sh
(
  set -eu
  printf 'Chemin absolu du dépôt : '
  IFS= read -r repo_path
  printf 'Cible complète confirmée (refs/heads/... ou refs/remotes/...) : '
  IFS= read -r target_ref
  report_dir="$(mktemp -d /private/tmp/wt-sweep-html.XXXXXX)"
  ./target/release/wt-sweep review --repo "$repo_path" --target "$target_ref" \
    --summary --disk-usage --html --run-dir "$report_dir"
  open "$report_dir"/run-*/report.html
)
```

Le dossier de sortie doit être extérieur aux repos inspectés. `--html` exige
`--run-dir` ; il ne modifie ni le JSON émis sur stdout ni les verdicts.
`plan --inventory` accepte la même option, avec une cible ou une carte par famille.
`scan --html` fournit un inventaire marqué **not reviewed**, sans verdict inventé.

## Lire le bilan

- Le haut du rapport indique le périmètre, les dates d’observation, la couverture
  et les totaux source/sélection. Ce sont des observations passées, pas un état
  courant vérifié au moment de l’ouverture.
- Le sommaire regroupe les worktrees par famille Git, même si deux projets ont
  le même nom. Les filtres CLI et le regroupement demandé restent consignés.
- Chaque ligne conserve chemin, branche ou tête détachée, HEAD complet, verdict,
  raisons, contrôles manquants et tailles. Les détails sont repliables au clavier.
- La recherche, le filtre de verdict et le tri changent les lignes visibles dans
  le navigateur. Les totaux source et les mesures disque ne sont pas recalculés.
- Les tailles sont affichées en Ko, Mo, Go, etc., en base décimale :
  1 Ko = 1 000 octets. L’affichage est arrondi à deux décimales ; les octets
  exacts restent disponibles au survol, dans les totaux dépliables et dans le
  JSON. Le tri utilise les octets exacts.
- Les octets logiques et alloués sont distincts. Une mesure absente ou partielle
  reste visible ; la somme des projets peut différer du total dédupliqué par inode.
  Aucun chiffre ne promet de l’espace récupérable sur APFS.

`cleanup_candidate` reprend la décision de la CLI. Il n’autorise aucune
suppression ; `apply` reste indisponible. Un reçu n’est pas une sauvegarde.

## Lire le bilan disque

Le bloc **Disk space review** apparaît en tête du rapport. Il distingue :

- **Observed disk footprint** : allocation dédupliquée des racines mesurées.
  Par défaut, toutes les racines enregistrées sont demandées, Git commun compris.
  Avec `--disk-root`, le total porte sur cette sélection explicite. Le libellé
  conserve la couverture partielle du périmètre source.
- **Recoverable space** : gain non établi. Les clones et snapshots APFS peuvent
  partager des blocs ; une taille allouée ne prédit pas l’espace libéré.
- **Cleanup review** : candidats et worktrees restant à vérifier dans le rapport
  source. Un inventaire seul indique que cette revue n’a pas été effectuée.

La couverture distingue les racines demandées et non demandées, puis les mesures
complètes, partielles et absentes. Une taille absente n’est pas un zéro. Le repli **Largest observed
worktree sizes** liste jusqu’à trois worktrees par allocation décroissante,
avec chemins copiables. Il exclut le Git commun ; les worktrees non mesurés
peuvent être plus grands. Les tailles peuvent se recouvrir et ne sont pas
additionnées. Vérifier le verdict de chaque worktree avant de préparer un
nettoyage. Les filtres du rapport ne changent ni ces chiffres ni ce classement.

## Thème et navigation

Le rapport utilise le thème BoldGuy : fonds crème en clair, presque noirs en
sombre, accent orange et métriques en monospace. Les verdicts conservent leurs
couleurs distinctes et leurs libellés. Le rapport utilise les polices du système ; ses styles sont
embarqués et aucun téléchargement n’est nécessaire.

Le thème suit le système au premier affichage. Le bouton permet de changer
le thème et mémorise ce choix lorsque le navigateur autorise le stockage local.
Si ce stockage est refusé, la bascule fonctionne pour la page courante. Sans
JavaScript, le rapport suit le thème du système et garde ses détails repliables.
Sur grand écran, déplier une preuve élargit sa colonne ; sur mobile, les
informations sont empilées. Le focus clavier est visible et les mouvements
sont réduits selon la préférence du système.

## Replier les grandes sections

Le bilan disque, la synthèse, les familles Git, les mesures disque et la
couverture s’ouvrent et se ferment en cliquant sur leur titre. Au clavier,
utilisez Tab puis Entrée ou Espace. Ces contrôles natifs fonctionnent sans
JavaScript. Les cinq sections sont ouvertes au chargement. Elles partagent le même fond,
les mêmes bordures et les mêmes marges internes dans les deux thèmes. Seul le
bilan disque conserve une bordure supérieure orange pour le repérer rapidement.

**Collapse all sections** et **Expand all sections** agissent uniquement sur
ces grandes parties : les preuves et autres détails gardent leur état. Ces
deux boutons nécessitent JavaScript, comme la réouverture automatique d’une
section depuis le menu ou un lien interne. Sans JavaScript, cliquez sur son
titre pour l’ouvrir. Le message de lecture seule reste visible, même lorsque
toutes les sections sont fermées. L’impression conserve le contenu des grandes
sections repliées. Aucune donnée ni sélection n’est supprimée par le repli.

Le contrôle navigateur facultatif accepte un rapport existant et un nouveau
dossier de captures, avec Playwright et Chromium déjà installés :

```sh
node tests/html-sections.cjs /chemin/vers/report.html /nouveau/dossier/captures
```

## Copier un chemin

Le bouton **Copy path** copie le chemin complet, avec ses espaces et accents,
depuis l’identité en octets enregistrée. Il apparaît dans les lignes de worktrees,
les familles Git, les observations disque et les détails de couverture. Les
boutons de la liste disque restent accessibles sans déplier chaque mesure.

**Path copied.** confirme la réussite. Si le navigateur refuse le presse-papiers,
une fenêtre propose le chemin déjà sélectionné : utilisez **⌘C** sur Mac,
**Ctrl+C** ailleurs, ou l’action Copier du téléphone. **Close** ou **Échap** ferme
la fenêtre et rend le focus au bouton.

Les chemins non UTF-8 ou contenant des caractères de contrôle ne sont pas copiés
en texte : le rapport l’indique et conserve leurs octets hexadécimaux dans les
preuves. Sans JavaScript, les boutons sont masqués et les chemins affichés
restent sélectionnables, avec leurs échappements visibles.

## Publication et limites

Le rendu est une fonction des observations déjà collectées, sans appel Git ni
nouvelle règle de décision. Les chemins conservent leur identité en octets ; les
balises, contrôles et noms hostiles sont affichés comme des données.

`run.json` lie la taille et l’empreinte SHA-256 de `report.html` comme celles du
texte et du JSON. Le plafond HTML est de 32 MiB. Un dépassement refuse la
publication avant de créer un run. Une erreur d’écriture/synchronisation HTML
ne publie pas de reçu réussi. Une erreur ultérieure de synchronisation du dossier
peut laisser un reçu visible sans établir la durabilité : conserver le code de
sortie et les diagnostics, pas seulement la présence de `run.json`.

Les tests de rendu, de publication et de CLI comparent les identités, verdicts,
empreintes et vues filtrées avec le JSON. Les vérifications de navigateur et le
SHA contrôlé sont consignés dans [le suivi de validation](progress.md).

## Vérifier le thème

Avec Node.js, Playwright et Chromium déjà disponibles, le contrôle facultatif
ci-dessous ouvre un rapport existant sans lancer de collecte. Utilisez un rapport
contenant au moins un worktree et un dossier de captures qui n’existe pas encore.

```sh
node tests/html-theme.cjs /chemin/vers/report.html /chemin/vers/nouvelles-captures
```

Si Playwright n’est pas accessible depuis le repo, `PLAYWRIGHT_MODULE` peut
pointer vers son installation existante. Le script n’installe rien. Il vérifie
douze rendus : 2560, 1440 et 390 px, clair et sombre, avec et sans JavaScript.
Il vérifie que le rapport utilise toute la largeur à droite du menu, puis teste la recherche, les filtres, le clavier, la préférence de thème après
rechargement, le stockage refusé, des contrastes de texte, les zones tactiles,
le mouvement réduit et le fond d’impression. Les captures et `checks.json`
sont écrits dans le dossier choisi. Relisez les captures : ces assertions ne
constituent pas un audit d’accessibilité exhaustif.

## Vérifier la copie

Le contrôle navigateur facultatif utilise la même installation Playwright :

```sh
node tests/html-copy.cjs /chemin/vers/report.html
```

Il ouvre un rapport local et instrumente le presse-papiers pour vérifier le texte
transmis, les refus, la sélection manuelle, le retour du focus et l’absence de
modification des identités ou preuves. Les cas incluent accents, espaces,
guillemets, octets non UTF-8 et caractères de contrôle. Il vérifie aussi la vue
mobile et l’affichage sans JavaScript. Cela ne vérifie pas le presse-papiers natif
de chaque navigateur : sur votre rapport, cliquez sur **Copy path**, puis collez
le résultat dans un éditeur de texte.


## Vérifier la disposition des chemins

Le contrôle suivant demande un rapport avec une mesure disque contenant au
moins une lacune associée à un chemin. Il réutilise Playwright sans relancer
la collecte et écrit dans un nouveau dossier.

```sh
node tests/html-layout.cjs /chemin/vers/report.html /chemin/vers/nouvelles-captures
```

Les quatre vues couvrent 1440 et 390 px, en clair et sombre. Les assertions
contrôlent la séparation entre chemin, bouton et motif, la largeur des détails
dépliés, les zones tactiles et l’absence de débordement. La copie du chemin
ne doit ni ouvrir ni fermer les détails ; le clavier doit conserver ce rôle.
Les captures et les mesures restent dans le dossier de sortie. Le script
demande une vraie lacune dans les données du rapport et échoue si elle manque.


## Aides du menu

Chaque entrée décrit le contenu de sa section : bilan et période, familles Git,
mesures disque, couverture et lacunes. Survolez le lien, placez-y le focus au
clavier ou utilisez le bouton **?**. Échap ou un clic à l’extérieur ferme l’aide.
Le lien continue de naviguer vers la section ; le bouton ouvre seulement l’aide.

Les boutons utilisent les popovers natifs du navigateur et restent utilisables
sans JavaScript dans Chromium testé. Le survol, le focus et le positionnement
près du lien sont ajoutés par JavaScript. Safari et Firefox ne sont pas vérifiés.

```sh
node tests/html-nav.cjs /chemin/vers/report.html /chemin/vers/nouvelles-captures
```

Ce contrôle réutilise Playwright et Chromium déjà installés. Il vérifie les
quatre explications en clair/sombre, sur desktop/mobile, avec et sans JavaScript :
survol, maintien sous le pointeur, focus, Échap, clic, ancres et limites du viewport.
