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
  ./target/release/wt-sweep review --repo "$repo_path" --target "$target_ref" \
    --summary --disk-usage --open
)
```

Sur macOS, `--open` implique `--html`, enregistre dans
`~/Desktop/Worktree Sweep/run-*/`, puis demande à l’application associée au HTML
d’ouvrir le rapport. `--html` seul enregistre sans ouvrir. Le Bureau doit déjà
exister ; Sweep crée seulement son dossier `Worktree Sweep`, avec accès privé.
Chaque exécution crée un nouveau sous-dossier sans écraser les rapports passés.

Ajoutez `--run-dir "/chemin/vers/rapports"` pour choisir un dossier existant,
extérieur au périmètre inspecté et aux dossiers Git. Si le Bureau est absent,
bloqué ou situé dans une racine choisie avec `--root`, utilisez une autre sortie.
La console affiche le chemin et une commande `Open HTML` pour le rouvrir.
L’ouverture suit la publication réussie ; son échec ne supprime aucun rapport
et reste un avertissement. Aucun navigateur n’est lancé sans `--open`.
Le JSON sur stdout et les verdicts restent inchangés.
`plan --inventory` accepte la même option, avec une cible ou une carte par famille.
`scan --html` fournit un inventaire marqué **not reviewed**, sans verdict inventé.

## Lire le bilan

Le résumé précède immédiatement la liste. **Browse worktrees** rejoint les
entrées enregistrées. **Largest measured first** trie globalement l'inventaire ;
dans une revue, **Sort each repository by size** trie à l'intérieur de chaque
repo, en conservant les familles Git.

**Quick filters** propose des raccourcis par état ou verdict. Un raccourci efface
la recherche incompatible ; **Show all** rétablit la liste et son tri initial.
Ces actions ne réintroduisent pas les lignes exclues d'une vue CLI sauvegardée.
Le résumé porte toujours sur la source, avec le nombre sélectionné indiqué
lorsqu'il diffère. Les dates et la provenance restent dans les détails.

La revue affiche **Keep**, **Needs checking** et **Possible cleanup**, avec une
raison principale lisible par ligne. **Why this status?**, **Commit identity** et
**Inspect evidence** conservent les verdicts CLI, tous les motifs, conditions,
OID, liens forge et JSON. Un candidat nécessite encore une vérification d'usage.
L'inventaire indique explicitement l'absence de revue Git.

- Pour un inventaire, le nombre d’inscriptions Git inclut les dossiers principaux,
  les worktrees liés, les dépôts sans dossier de travail et les chemins manquants.
  Le résumé distingue dossiers de travail présents, dépôts sans dossier de travail
  (`bare`) et chemins absents ou non vérifiés. Ce nombre n’est ni un total de
  worktrees supplémentaires ni une garantie de découverte exhaustive.
- L’inventaire commence par les décomptes puis une liste ouverte, sans en-tête
  répété pour chaque repo. Elle affiche repo, branche, chemin complet, taille
  mesurée et état du dossier. Les dossiers présents viennent en premier ; les
  dépôts sans dossier de travail, les chemins absents et ceux non vérifiés restent
  dans la liste. Les détails de chaque ligne conservent le chemin Git commun,
  son identité physique, le HEAD complet et les faits enregistrés.
- La recherche et le filtre d’état portent sur cette liste ; le tri des tailles
  est global et utilise les octets exacts, avec les tailles inconnues après les
  mesures, y compris une mesure nulle. Le bilan source ne change pas avec les filtres.
- Les rapports de revue conservent le regroupement par famille Git, même si deux
  projets ont le même nom. Les filtres CLI et le regroupement demandé restent consignés.
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

Le dashboard apparaît une seule fois, dans **Disk observations**, après la
liste et replié au chargement. Il présente l'allocation observée, la mention
partielle si nécessaire et les mesures source, indépendamment des filtres.
Une mesure partielle laisse le total de l'inventaire inconnu. Avec `--disk-root`,
les mesures portent sur la sélection explicite.

Deux jauges par volume montrent la capacité moins l'espace disponible, puis
l'allocation mesurée divisée par la capacité. Les volumes sont séparés ; une
capacité absente ne devient pas un pourcentage. Le rendu ne consulte pas la
machine pour compléter un ancien fichier. Les [mesures disque](disk-usage.md#lire-les-jauges-html)
donnent les formules et les limites APFS. Aucun chiffre ne promet l'espace libéré
par un nettoyage ; les clones et snapshots peuvent partager des blocs.

La couverture distingue les racines demandées et non demandées, puis les mesures
complètes, partielles et absentes, séparément pour les dossiers de travail et le
Git partagé. Leur nombre combiné n’est pas un nombre de worktrees. Une taille
absente n’est pas un zéro. Le repli **Largest observed
worktree sizes** liste jusqu’à trois worktrees par allocation décroissante,
avec chemins copiables. Il exclut le Git commun ; les worktrees non mesurés
peuvent être plus grands. Les tailles peuvent se recouvrir et ne sont pas
additionnées. Vérifier le verdict de chaque worktree avant de préparer un
nettoyage. Les filtres du rapport ne changent ni ces chiffres ni ce classement.

## Guide de lecture et recherche rapide

Le bloc visible « In brief: recommended actions » précède la liste. Dans un
inventaire, il propose d'inspecter les gros dossiers, de lancer une revue Git
avec dépôt et cible explicites, puis de compléter la découverte si une
frontière de parcours est enregistrée. Un exemple de commande reste replié.
Dans une revue, les actions dépendent des verdicts enregistrés : confirmer
l'usage des candidats, résoudre les preuves manquantes et préserver les
entrées `keep`. L'absence de candidats et une revue vide sont explicites.
Ces conseils n'ajoutent aucune preuve ni autorisation de suppression.

L'aide facultative « How to use this report », placée après la liste, distingue
inventaire et revue Git. Son
schéma Mermaid est fourni en SVG statique dans le HTML : il fonctionne hors
ligne, sans JavaScript ni chargement de Mermaid. Il décrit le parcours,
pas des étapes toutes réalisées dans le rapport courant. Les icônes SVG du
menu et des titres complètent les libellés, sans les remplacer.

Avec JavaScript, « Search report », ⌘K sur Mac ou Ctrl+K ailleurs ouvre une
recherche des sections et des lignes enregistrées. Les chemins, repos et
branches sont recherchés dans les données de la liste ; les incidents du
système de fichiers ne remplissent pas ces résultats. Les flèches déplacent
la sélection, Entrée rejoint le résultat, Échap ferme et rend le focus.
Sélectionner un worktree efface les filtres de la liste, ouvre la section et
ses preuves, puis place le focus sur la ligne. Aucun fait source ni total disque
n'est modifié. La palette affiche au plus trente résultats à la fois avec le
nombre total de correspondances ; préciser la recherche donne accès aux autres.
Sans JavaScript, les liens, la liste, le guide et les détails restent accessibles.

« Scan limits » présente les limites de découverte et de collecte. Il conserve
aussi les HEAD inconnus et les inventaires locaux incomplets. Une frontière de
découverte enregistrée indique les chemins restants ; un indicateur incomplet
sans cette frontière ne devient pas une preuve de parcours disque interrompu.
Les chemins restants ne sont pas un nombre de worktrees non découverts. Les
incidents de liens, d'accès et autres raisons sont regroupés dans les détails
techniques repliés. Chaque chemin, raison et bloc JSON original est conservé.
Un incident lié à une application ou à un framework n'est pas un verdict Git.

```sh
node tests/html-command.cjs /chemin/vers/fixture.html /chemin/vers/nouvelles-captures
```

Le SVG est généré depuis `src/presentation/html/report-flow.mmd` et sa
configuration avec un Mermaid CLI déjà installé. Pour le reproduire :

```sh
mmdc -i src/presentation/html/report-flow.mmd -o /tmp/report-flow.svg \
  -c src/presentation/html/report-flow-config.json -b transparent \
  -I wt-report-flow -q
```

Les sept blocs sont des liens internes dans le même onglet. Les blocs de revue
ouvrent les explications dans un inventaire, ou les preuves de la liste dans
un rapport revu. Les liens fonctionnent au clic et avec Tab/Entrée ; le script
du rapport ouvre aussi une destination repliée lors d'un clic répété.

Le mode `loose` sert uniquement à générer les liens du diagramme fixe, sans
contenu capturé. Aucun moteur Mermaid ne s'exécute dans le rapport. Le fichier
intégré ajoute un titre accessible, `role="group"` et `aria-labelledby` pour
exposer ses liens. Les attributs `xlink:href` deviennent `href` ; chaque lien
reçoit `tabindex="0"` et un `aria-label` égal au texte de son bloc. Ces ajustements
préservent les liens de fragment générés et les rectangles du diagramme.
Chaque lien ajoute hors du groupe visible un rectangle transparent de 80 unités
de haut, centré sur son bloc, pour conserver une cible d’au moins 44 px à 390 px.
Le `viewBox` ajoute 16 unités en haut et en bas pour ne pas rogner ces cibles.
Le test navigateur vérifie leur taille et la navigation dans les deux thèmes.
Les refus d'accès du navigateur de génération ne demandent aucune installation :
la génération locale peut utiliser un Chromium existant avec `-p CONFIG.json`.

```sh
node tests/html-diagram.cjs /chemin/vers/inventaire.html /chemin/vers/nouveaux-resultats /chemin/vers/revue.html
```

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

La synthèse, la liste, les mesures disque et les limites s'ouvrent et se
ferment avec leur titre, ou Tab puis Entrée/Espace. Dans les deux modes, seuls
le résumé et la liste sont ouverts au chargement. Les contrôles natifs et les
preuves restent utilisables sans JavaScript.

Dans **Display options**, **Collapse all sections** et **Expand all sections**
agissent sur ces quatre parties ; les preuves et autres replis gardent leur
état. Ces boutons et la réouverture depuis un lien nécessitent JavaScript.
Sans JavaScript, le titre permet d'ouvrir la section. La mention de lecture
seule reste dans l'en-tête. L'impression conserve les grandes sections
repliées. Aucune donnée ni sélection n'est supprimée.

Le contrôle navigateur facultatif accepte un rapport existant et un nouveau
dossier de captures, avec Playwright et Chromium déjà installés :

```sh
node tests/html-sections.cjs /chemin/vers/report.html /nouveau/dossier/captures
```

Le contrôle suivant vérifie les filtres et tris du script avec des lignes
synthétiques, sans navigateur, collecte Git ou disque :

```sh
node tests/html-list.cjs
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
mesures disque, couverture et lacunes. L’intitulé est le seul contrôle : son
survol, son focus au clavier ou son clic affiche l’explication. Le clic continue
de naviguer vers la section. Échap ou un clic à l’extérieur ferme l’aide.

Avec JavaScript, un popover positionne l’aide près du lien. Sans JavaScript,
l’attribut `title` conserve une aide native au survol et les liens naviguent
normalement. Le comportement est vérifié dans Chromium ; Safari et Firefox
ne sont pas vérifiés.

```sh
node tests/html-nav.cjs /chemin/vers/report.html /chemin/vers/nouvelles-captures
```

Ce contrôle réutilise Playwright et Chromium déjà installés. Il vérifie les
quatre liens en clair/sombre, sur desktop/mobile, avec et sans JavaScript. Avec
JavaScript, il contrôle le survol, le maintien sous le pointeur, le focus, Échap,
le clic et les limites du viewport. Sans JavaScript, il vérifie les ancres et
la présence de l’attribut `title`, sans observer l’infobulle native elle-même.

## Liens GitHub et GitLab

Dans une revue, dépliez **Inspect evidence**, puis **Repository links**, pour
ouvrir le dépôt, la branche ou le commit des remotes identifiés. Les remotes sont nommés séparément ; le lien
de branche utilise l’upstream configuré quand il est connu. Une égalité avec la
ref locale de suivi reste une observation locale, sans preuve de push actuel.

Les PR/MR précisément associées par `--forge FILE` affichent leur titre, leur
état capturé et un lien direct. Leur provenance est dépliable. Aucun appel réseau
n’est ajouté à la collecte ni au chargement du HTML. Un clic ouvre la forge dans
un nouvel onglet. Les routes et libellés sont validés et échappés.

`--no-context` omet les liens issus des remotes, sans retirer les PR/MR importées.
Les anciens rapports et `scan --html` ne reçoivent pas de données inventées.
Voir [le contrat des liens](forge.md#links-in-review-reports), notamment les
limites sur les forks GitLab et les instances auto-hébergées.

## Vérifier le parcours de lecture

Les fixtures exportées par les tests Rust restent synthétiques. Le contrôle
suivant ouvre les deux modes à 1440 et 390 px, en clair et sombre, avec et sans
JavaScript. Il vérifie la position de la liste, l'ouverture au clavier, les
raccourcis, le reset et la conservation des données après filtrage.

```sh
node tests/html-shortcuts.cjs
node tests/html-ux.cjs /chemin/vers/inventaire-synthetique.html \
  /chemin/vers/revue-synthetique.html /chemin/vers/nouvelles-captures
```

Ces scripts utilisent un navigateur déjà installé ; ils ne déclenchent aucune
collecte ni suppression. Les captures synthétiques ne prouvent pas le rendu de
chaque inventaire personnel.
