`report.html` est autonome : styles et script embarqués, sans modèle ni ressource réseau. Ouvrez-le directement dans le navigateur. Synthèse et preuves restent lisibles sans JavaScript.

[Ouvrir l’exemple synthétique](/demo/report.html).

## Ordre de lecture

1. Périmètre, intervalles d’observation, couverture et totaux source/sélection.
2. Familles Git et verdict de chaque worktree.
3. Motifs de conservation, conditions manquantes et preuves dépliées.
4. Lignes disque et mesures complètes, partielles ou absentes.

Recherche, filtres de verdict et tri changent les lignes visibles, sans recalculer les totaux source ni les mesures disque. Sélection CLI et filtre du navigateur restent distincts.

## Chiffres disque

**Observed disk footprint** est l’allocation dédupliquée des racines demandées et mesurées. **Recoverable space** reste non établi. **Cleanup review** décrit la revue source, ou indique qu’elle n’a pas été effectuée pour un simple inventaire.

Les unités sont décimales et l’affichage est arrondi ; les octets exacts restent au survol et dans le JSON. La liste des plus gros worktrees observés exclut le Git partagé et peut omettre de plus grosses racines non mesurées. N’additionnez pas ses tailles susceptibles de se recouvrir.

## Navigation et partage

Utilisez les sections et preuves repliables natives au clavier. Le thème suit le système, avec un choix local mémorisé facultatif. La copie utilise l’identité du chemin et propose un repli manuel si le presse-papiers est refusé. Les chemins non UTF-8 ou contenant des contrôles conservent leurs octets réversibles plutôt qu’une copie texte altérée.

Gardez `run.json`, JSON et texte avec le HTML. Vérifiez tailles et SHA-256 des artefacts contre le reçu avant de partager. Un reçu n’est ni une sauvegarde, ni une preuve de synchronisation finale réussie. Un rapport historique n’est pas une revue fraîche de l’état actuel.

Les [captures datées de la landing](/assets/report-captures.json) et la [provenance de la nouvelle démo interactive](/demo/provenance.json) sont des relevés distincts.
