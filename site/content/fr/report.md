`report.html` est autonome : styles et script embarqués, sans modèle ni ressource réseau. Ouvrez-le directement dans le navigateur. Le bilan et les sections de détail restent lisibles sans JavaScript.

[Ouvrir l’exemple synthétique](/demo/report.html).

## Ordre de lecture

1. Périmètre, intervalles d’observation, couverture et totaux de l’analyse complète et de la sélection.
2. Familles Git et verdict de chaque worktree.
3. Motifs de conservation, conditions manquantes et détails des preuves.
4. Lignes disque et mesures complètes, partielles ou absentes.

Recherche, filtres de verdict et tri changent les lignes visibles, sans recalculer les totaux source ni les mesures disque. Sélection CLI et filtre du navigateur restent distincts.

## Chiffres disque

**Observed disk footprint** est l’allocation dédupliquée des racines demandées et mesurées. **Recoverable space** indique que l’espace récupérable n’a pas été établi. **Cleanup review** décrit l’analyse complète, ou indique qu’elle n’a pas été effectuée pour un simple inventaire.

Les unités sont décimales et l’affichage est arrondi ; les octets exacts restent au survol et dans le JSON. La liste des plus gros worktrees observés exclut le Git partagé et peut omettre de plus grosses racines non mesurées. N’additionnez pas ses tailles susceptibles de se recouvrir.

## Navigation et partage

Vous pouvez ouvrir et fermer les sections de détail au clavier. Le thème suit celui du système ; vous pouvez aussi choisir un thème enregistré localement. La copie respecte l’identité du chemin et propose une copie manuelle si l’accès au presse-papiers est refusé. Les chemins non UTF-8 ou contenant des caractères de contrôle sont décrits sous une forme qui conserve leurs octets, sans copie texte altérée.

Gardez `run.json`, les fichiers JSON et le rapport texte avec le HTML. Avant de partager, comparez les tailles et SHA-256 des fichiers avec le reçu de publication `run.json`. Un reçu n’est ni une sauvegarde, ni une preuve de synchronisation finale réussie. Un ancien rapport ne décrit pas forcément l’état actuel des worktrees.

Les [captures datées de la page d’accueil](/assets/report-captures.json) et le [relevé de génération de la démo interactive](/demo/provenance.json) sont des relevés distincts.
