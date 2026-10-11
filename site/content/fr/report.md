`report.html` est autonome : styles et script embarqués, sans modèle ni ressource réseau. Ouvrez-le directement dans le navigateur. Le bilan et les sections de détail restent lisibles sans JavaScript.

[Ouvrir l’exemple synthétique enregistré](/demo/report.html). Cet exemple précède le tableau de bord par volume décrit ci-dessous.

## Tableau de bord disque et liste

Disponible dans `v0.1.0-alpha.2`. Les deux rapports ouvrent un bilan compact,
**In brief: recommended actions** et la liste des worktrees. Les recommandations
suivent le périmètre et les verdicts enregistrés. Les mesures disque et les
détails de couverture sont repliés sous la liste.

Les lignes analysées affichent **Keep**, **Needs checking** ou **Possible cleanup**
et le motif principal. Dépliez une ligne pour retrouver le verdict original,
les identifiants complets et les preuves. Les filtres rapides sélectionnent un
statut, le raccourci de taille trie les dossiers mesurés et **Show all** rétablit la vue
initiale. **Possible cleanup** exige encore une vérification d’usage et n’autorise
aucune suppression.

Avec `--disk-usage`, le tableau de bord affiche la capacité capturée, l’espace disponible et l’allocation mesurée pour chaque volume. Les volumes restent séparés, y compris les volumes APFS partageant un conteneur. Un ancien inventaire conserve ses tailles et indique la capacité manquante ; le rendu n’interroge jamais la machine actuelle.

L’inventaire ouvre directement la liste des worktrees. Filtrez les entrées présentes, bare, absentes ou non vérifiées, ou triez toute la liste. L’analyse conserve les familles Git et les filtres de verdict. Cmd+K / Ctrl+K ouvre une recherche locale parmi les sections et les worktrees enregistrés. Le guide de lecture et son diagramme fonctionnent hors ligne ; les limites techniques du scan restent dépliables.

Le diagramme comporte sept blocs cliquables. Un clic, ou Tab puis Entrée, rejoint la section correspondante. Dans un inventaire, les blocs d’analyse expliquent quelles vérifications Git manquent. Dans un rapport analysé, ils ouvrent les preuves des worktrees. Les liens fonctionnent sans JavaScript ; avec JavaScript, un clic répété rouvre aussi une destination repliée.

Les filtres ne changent ni les comptes source, ni les jauges, ni les preuves. Une mesure partielle reste partielle ; l’allocation mesurée ne prédit pas l’espace libéré.

## Enregistrer et ouvrir un rapport

Ajoutez `--open` à `review`, `scan` ou `plan` pour enregistrer le HTML et demander à macOS de l’ouvrir après publication. Sans `--run-dir`, la sortie est `~/Desktop/Worktree Sweep/run-*/` ; le Bureau doit déjà exister. Utilisez `--html` pour enregistrer sans ouvrir, ou un `--run-dir` explicite hors des racines inspectées. Chaque exécution crée son propre dossier.

## Liens GitHub et GitLab

Dans une analyse, dépliez **Inspect evidence**, puis **Repository links** pour ouvrir le dépôt, la branche ou le commit enregistré. Ces URL viennent des remotes locaux reconnus. Une ref locale de suivi identique à HEAD ne prouve ni un push ni l’état actuel du serveur. La CLI ne contacte pas la forge.

Les liens PR/MR associés sont visibles avec leur état capturé ; leurs détails donnent le titre et la provenance. Ils exigent un import `--forge FILE` correspondant. Les instances auto-hébergées exigent aussi un import associé pour identifier leur fournisseur. Les remotes inconnus ou ambigus restent signalés. `--no-context` omet leurs métadonnées mais conserve les PR/MR déjà associées. Un simple inventaire ne collecte pas ces liens.

Ouvrir un lien externe contacte la forge avec votre navigateur, sans changer le verdict. [Format des imports et règles d’association](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/forge.md#links-in-review-reports).

## Ordre de lecture

1. Périmètre, intervalles d’observation, couverture et totaux de l’analyse complète et de la sélection.
2. Familles Git et verdict de chaque worktree.
3. Motifs de conservation, conditions manquantes et détails des preuves.
4. Lignes disque et mesures complètes, partielles ou absentes.

Recherche, filtres de verdict et tri changent les lignes visibles, sans recalculer les totaux source ni les mesures disque. Sélection CLI et filtre du navigateur restent distincts.

## Chiffres disque

**Observed disk allocation** est l’allocation dédupliquée des racines mesurées. Capacité et disponibilité du volume sont des valeurs capturées séparément. Les comptes de nettoyage apparaissent uniquement pour une analyse ; un inventaire seul n’a aucun verdict de nettoyage. L’espace récupérable reste inconnu.

Les unités sont décimales et l’affichage est arrondi ; les octets exacts restent au survol et dans le JSON. La liste des plus gros worktrees observés exclut le Git partagé et peut omettre de plus grosses racines non mesurées. N’additionnez pas ses tailles susceptibles de se recouvrir.

## Navigation et partage

Les intitulés de la barre latérale expliquent leur contenu au survol, au focus ou au clic. Vous pouvez ouvrir et fermer les sections de détail au clavier. Le thème suit celui du système ; vous pouvez aussi choisir un thème enregistré localement. La copie respecte l’identité du chemin et propose une copie manuelle si l’accès au presse-papiers est refusé. Les chemins non UTF-8 ou contenant des caractères de contrôle sont décrits sous une forme qui conserve leurs octets, sans copie texte altérée.

Gardez `run.json`, les fichiers JSON et le rapport texte avec le HTML. Avant de partager, comparez les tailles et SHA-256 des fichiers avec le reçu de publication `run.json`. Un reçu n’est ni une sauvegarde, ni une preuve de synchronisation finale réussie. Un ancien rapport ne décrit pas forcément l’état actuel des worktrees.

Les [captures datées de la page d’accueil](/assets/report-captures.json) et le [relevé de génération de la démo interactive](/demo/provenance.json) sont des relevés distincts.
