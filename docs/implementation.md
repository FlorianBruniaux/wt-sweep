# Implémentation et ownership

[Index documentaire](README.md) · [Contribution](../CONTRIBUTING.md)

Lire le [plan](plan.md) et les [résultats locaux](progress.md). Le présent document
définit les lots et responsabilités ; leur validation exige les preuves du
registre. Garder la comparaison des alternatives ouverte avant de développer le
moteur de nettoyage.

## Démarrage

L'intégrateur définit le contrat d'inventaire et une fixture attendue non vide.
Les types communs entrent quand leur premier consommateur existe, sans générer
une arborescence de modules vides. Un crate suffit ; introduire clap, serde ou
une bibliothèque d'empreintes au moment où une unité les exige.

| Lot | Propriétaire | Périmètre proposé | Fin observable |
|---|---|---|---|
| U1 collecte | A | Collecteur Git et tests associés | Inventaire exact d'une famille |
| U1 adversarial | B | Fixtures temporaires, oracle et tests de lecture seule | Effets de bord détectés et erreurs non masquées |
| E1 | C | Expériences isolées, décision documentée | Exclusion démontrée ou application bloquée |
| Contrats et raccordements | I | Cargo, CLI, types communs, docs | Interfaces partagées et suite intégrée verte |

Après U1 : A prend U2, B prend U3, C peut préparer U4 après E1. U4 rejoint les
sorties U2/U3. U5 commence seulement après U3 et E1 concluante. Ses lots fichiers,
objets Git et durabilité peuvent être séparés une fois le contrat de restauration
figé. U6 garde un unique propriétaire de la machine d'états ; les autres ajoutent
restauration et fautes adversariales. U7 reste une exécution bornée séquentielle.

Lors de cette exécution, B conserve son rôle de test indépendant pour U3 ;
l'intégrateur écrit `src/review.rs`. A utilise GPT-6.1 Sol avec raisonnement
`high` pour U1/U2, B le même modèle/niveau pour les oracles, C GPT-6 Astra `high`
pour E1/U4 et la revue d'U1/U3. A relit U4. Aucun auteur n'approuve son propre lot.
Le checkout reste partagé ; aucun commit n'a été effectué pendant la reprise.

Pour clôturer U2, A prend `src/mounts.rs` et ses tests, puis le correctif Git des
administrations séparées. C prend `src/residency.rs` et ses tests ; A et C relisent
mutuellement ces frontières natives. L'intégrateur reprend seul la découverte,
son parallélisme et les contrats partagés. B ajoute les oracles MVP et le rejeu
historique, sans modifier le code produit. Les modèles et niveaux restent ceux
définis ci-dessus.

La finition issue du RETEX conserve cette répartition : A possède `local.rs`,
les lectures Git associées et `tests/local.rs` ; B possède les oracles indépendants
de `tests/test_enriched.py` ; C examine les capacités E1 puis relit les nouveaux
lots. L'intégrateur possède les types communs, le raccordement CLI, le rapport
descriptif et les sorties par exécution. Voir le [registre de finition](finish-ledger.md).

## Règles de coopération

- Maximum initial : trois exécutants plus l'intégrateur, dans les limites du
  client. Ce plafond n'est pas une mesure d'optimalité. Ne déléguer qu'avec une
  demande ou une autorisation applicable, pas pour occuper un créneau.
- Un propriétaire par fichier. I seul modifie Cargo et les contrats partagés.
  Passer une demande d'interface à I ; ne pas créer une copie concurrente du type.
- Utiliser branches `codex/<lot>` et checkouts isolés après un premier commit de
  base accepté. Les fixtures Git ne sont jamais les worktrees de développement.
- Chaque test possède son dossier et ses processus. Ne pas partager cache ou
  sortie mutable ; arrêter et attendre les processus créés par le test.
- Remettre fichiers concernés, échec initial, résultat final, limites et prochain
  raccordement. La revue d'un lot est faite par un autre auteur si une revue
  indépendante est requise. Les commits et effets distants suivent l'autorisation
  de la tâche, jamais la seule réussite des tests.

Les tests de découverte globale injectent leurs volumes et racines : ils ne
parcourent pas réellement la machine. Les sorties Git de preuve restent brutes,
avec code de sortie contrôlé. Les chemins non UTF-8 et l'absence d'objet doivent
être représentables sans conclure par défaut au succès.
