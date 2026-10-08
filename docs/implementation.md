# Implémentation et ownership

[Index documentaire](README.md) · [Contribution](../CONTRIBUTING.md)

Lire la [feuille de route](roadmap.md), le [plan](plan.md) et les
[résultats locaux](progress.md). Le présent document
définit les lots et responsabilités ; leur validation exige les preuves du
registre. Garder la comparaison des alternatives ouverte avant de développer le
moteur de nettoyage.

## Incréments du 07/10, livrés le 08/10/2026

La PR #15 livre le [rapport HTML natif](html-report-plan.md) dans les runs
`scan`, `review` et `plan`, et les premiers faits provisoires d’histoire et
d’état local dans la progression. Le résumé local interprète aussi
intent-to-add dans les index SHA-1 et SHA-256. Le [registre](progress.md) et
les issues du [jalon](roadmap.md) distinguent validation locale, SHA livré,
CI et recette réelle. La présence des sources ne constitue pas une release binaire.

Le [runner de mesure](../scripts/benchmark-review.py) compare des binaires
figés sur des familles synthétiques. Le [rapport de comparaison runtime](research/worktree-runtime-comparison-2026-10-07.md)
consigne les 48 audits natifs de Sweep, Holt et AgentRinse, avec leurs contrats
différents. Les mesures avant/après de collecte ont leur registre séparé ; aucun
gain chiffré n’est déduit de la présence du runner. Ces lots ne couvrent
ni gestion d’environnements ni retrait.

### Répartition du lot audit

| Responsabilité | Agent et paramètres de lancement | Périmètre |
| --- | --- | --- |
| Rendu HTML | Rust, GPT-6.1 Sol `high` | Projection pure, styles, interactions et tests de rendu. |
| Collecte et mesures | Implémentation Sweep, GPT-6.1 Sol `xhigh` | Index, histoire privée, observations progressives et corpus constants. |
| Comparaison des outils | Architecture backend, GPT-6.1 Sol `high` | Copies jetables, CLI natives et oracle de conservation. |
| Documentation et prompt | Rédaction technique, GPT-6.1 Sol `medium` | Guides, liens, aide et lecture d’un rapport synthétique. |
| Revue indépendante | GPT-6 Astra `high` | Contrats, limites, correctifs de revue et protocole de mesure. |
| Intégration | Coordinateur | CLI, types communs, publication, projections, suites et livraison. |

Trois sous-agents au plus travaillent avec le coordinateur ; les rôles se
relaient quand un lot se termine. Chaque lot possède ses fichiers. Les suites
complètes et les mesures lourdes sont sérialisées pour éviter les conflits et
la concurrence CPU pendant les benchmarks. Ce tableau décrit les
paramètres de lancement, pas un classement mesuré des modèles.

## Collecte bornée A1-A6

L’intégrateur possède les profils `standard|thorough`, les contrats optionnels de
métriques et le raccordement CLI. Les collecteurs gardent leurs limites finies,
les vérifications d’OID et les revalidations. Le lot documentaire aligne CLI,
tutoriel et skill sur le passage standard, la lecture des faits sauvegardés puis
la reprise ciblée thorough. Une augmentation de budget n’est pas une preuve de
complétude ni une autorisation de retrait.

Le skill reste limité au dépôt. La source canonique est
`harness/skills/wt-sweep/SKILL.md` avec ses références ; l’intégrateur génère les
projections Claude et Codex et vérifie leur routage. Les mesures et contrôles
observés restent dans le [registre de collecte](collection-performance.md),
séparés des plafonds définis par le code.

## Triage T01-T09

Ces lots sont fusionnés dans `main` depuis le 06/10/2026. Le lot suivant corrige
les lectures unitaires d’objets locaux et les parcours répétés des cibles Git.
Les lectures restent bornées ; les preuves sont dans le
[registre de collecte](collection-performance.md), avec les critères de sortie
dans la [feuille de route](roadmap.md#collecte-des-gros-dépôts--a1-a6).

La reprise de triage s’appuie sur le [plan v2](superpowers/plans/2026-10-05-worktree-triage-v2.md).
Elle ajoute cibles par famille, histoire du graphe, résumé local courant, histoire
privée, décisions C1-C5, contexte descriptif, vues et collecte bornée. T09 raccorde
`review`, l’aide par commande et la publication de l’inventaire original avec la
revue. Le [checkpoint courant](finish-ledger.md#triage-t01-t09) porte les preuves
finales ; les lots et chiffres du socle ci-dessous restent historiques.

L’intégrateur possède CLI, contrats, Cargo, changelog et raccordements. Le lot T09
confie le parseur de graphe et les reçus à son exécutant, l’oracle CLI à un auteur
distinct et les guides à la rédaction technique. Le checkout est partagé ; chaque
lot conserve les modifications des autres. Une validation documentaire ou un
compteur de tests ne certifie ni activation native ni nettoyage réel.

## Démarrage historique U1-U4

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
