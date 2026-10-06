# Retours des sessions de ménage : apports à Worktree Sweep

[Index documentaire](README.md) · [Guide de démarrage](getting-started.md)

Lecture et expériences du 04/10/2026. Sources : `RETEX-session-msds-20261004.md`,
les cinq scripts et les sept TSV conservés sous `scripts/session-msds/` dans le
dossier de ménage, ainsi que le texte Aristote fourni dans cette conversation.
Ce dernier contient des passages tronqués : ils ne sont pas reconstruits.
Les inventaires partagés signalés comme écrasés ne sont pas utilisés. Aucun
dépôt réel, sauvegarde contenant des secrets ou état distant n'a été inspecté.

Le retour msds rapporte 134 suppressions sur 166, sans perte constatée. Les trois
plans exécutés contiennent bien 46, 38 et 50 lignes, soit 134 chemins distincts.
Cela confirme la cohérence de la sélection, pas la restauration de chaque donnée.
Le texte Aristote décrit un autre périmètre, 34 suppressions sur 65 ; les deux
résultats ne sont pas fusionnés. Les volumes libérés, actions de forge et absence
de perte restent des résultats rapportés par leurs sessions respectives.

## Hypothèses éprouvées localement

Les nouveaux témoins sont dans `experiments/exclusion/test_retex.py`. Ils créent
leurs dépôts sous `/private/tmp`, sans argument permettant de viser un dépôt
réel. Git est isolé des configurations et hooks utilisateur, les processus
enfants et commandes sont bornés. La fixture et une sentinelle extérieure sont
contrôlées. Aucun `--force`, déverrouillage ni suppression de branche n'est utilisé.

Résultats avec Git 2.56.0, puis les cinq nouveaux tests rejoués avec
`/usr/bin/git` 2.50.1 (Apple Git-155), sous macOS 26.5.1 arm64 :

| Hypothèse ou recette | Observation | Conséquence |
|---|---|---|
| H1 : retrait sans force et fichiers ignorés | Un worktree propre contenant des octets ignorés est retiré sans `--force` ; ces octets disparaissent. Sa branche et la sentinelle restent. Un fichier non suivi ordinaire provoque au contraire un refus et reste présent. | Le succès de Git ne prouve pas la conservation des fichiers ignorés. |
| H4 : absence de cwd dans la cible | Sur le seul enfant possédé par le test, `lsof` montre un cwd extérieur et un descripteur ouvert dans la cible. L'écriture via ce descripteur réussit. | Un filtre cwd ne couvre pas les écrivains. Une vue native refusée serait UNKNOWN, jamais une absence. |
| Empreinte HEAD + status | Modifier successivement les octets d'un fichier déjà modifié, d'un non-suivi et d'un ignoré laisse identiques les octets HEAD/status et leur SHA-1 tronqué. Les SHA-256 des contenus changent. | L'empreinte des scripts ne détecte pas « tout changement ». Utiliser un hash plus long sur les mêmes entrées ne corrigerait pas ce défaut. |
| Patch `git diff HEAD --binary` | Le patch restaure les octets sur disque dans un worktree neuf. Une version différente qui était dans l'index disparaît : l'index restauré reste celui de HEAD. | Patch et archive de fichiers ne suffisent pas à restituer l'état de travail complet. |

Ces cinq nouveaux tests passent, avec le témoin E1 existant. Ils démontrent ces
contre-exemples précis. Ils ne valident ni un moteur de retrait ni les anciennes
sauvegardes. H2, « la liste des régénérables est suffisante », reste non démontrée.
H3, « le numéro dans le nom identifie la bonne MR », reste une heuristique sans
autorité : il faut hôte, projet et identité du commit. Le cas historique déjà
rejoué `lead-app-mr43` associé à !383 illustre aussi la fragilité du nom.

## Décision sur les huit pistes

| Piste du retour | À reprendre dans l'outil | État ou condition |
|---|---|---|
| 1. Données isolées par session | Un répertoire unique par exécution, des fichiers nouveaux et une identité d'exécution explicite. | `--run-dir` crée JSON, détail et reçu dans un dossier nouveau ; aucun verrou coopératif n'est présenté comme exclusion générale. |
| 2. Empreinte avec ignorés | Inventorier contenus, types, liens et état de l'index, avec erreurs explicites. | `scan --local` lie métadonnées et hashes dans un manifeste borné. Index partagé conservé brut, interprétation inconnue ; ACL/xattrs et conservation complète restent hors contrat. Le manifeste ne ferme pas la fenêtre E1. |
| 3. Preuve en trois étages | Garder patch, titre et mention de CHANGELOG comme indices descriptifs. | Rejeter leur promotion en preuve. Les relations actuelles restent identité de commit, ascendance brute et identité d'arbre, avec cible et OID explicites. Une branche de MR ouverte n'est pas automatiquement la cible d'intégration. |
| 4. Branche d'archive détachée | Signaler un HEAD détaché et les références connues ; conserver refs et objets dans la future sauvegarde. | Aucune écriture automatique dans le MVP. Une branche nommée ne préserve ni index, ni fichiers locaux, ni reflog propre au worktree. Une ancre ne remplace pas une restauration sans source. |
| 5. Verrou Claude périmé | Présenter la raison du verrou comme observation et l'identité de processus comme non vérifiée. | Le verrou Git est déjà collecté. Un PID absent ou réutilisé, ou une commande refusée, ne justifie aucun déverrouillage automatique. |
| 6. Détail et récap court | Décrire le travail avant de demander une décision : sujet, branche/HEAD, MR et relation observée ; fournir un récap de cinq lignes maximum avec accès au détail. | Rapport descriptif et `plan --summary` raccordés. Le 06/10, le récap passe à plusieurs blocs pour distinguer les verdicts et la couverture. Sujet et titre sont bornés et échappés, sans autorité sur le jugement. |
| 7. Journal par passage | Identifier chaque exécution et conserver ses événements et résultats séparément. | Le reçu `run.json` lie les résultats de lecture seule ; il ne constitue pas le futur journal transactionnel d'application. |
| 8. Secrets dans les archives | Signaler les chemins potentiellement sensibles et les garanties de protection de la sauvegarde. | Futur U5. Ne jamais afficher leurs valeurs ; noms et détection ne garantissent pas un inventaire exhaustif. Aucun accès aux sept fichiers signalés n'a été effectué ici. |

## Limites supplémentaires des scripts

- `wt-plan.sh` et `wt-apply.sh` masquent les erreurs Git dans certaines pipelines.
  Sans contrôle de chaque statut, un échec peut produire une empreinte ou un
  compteur apparemment valide. Le produit garde son exécuteur contrôlé.
- `tar -t` valide la lisibilité de la liste d'une archive. Il ne démontre ni
  exhaustivité, ni durabilité, ni restitution des index, ACL, attributs, reflogs
  et objets. Le test du patch montre déjà une perte d'état possible.
- Les listes TSV et ignorés séparés par `|`, le parsing de `.git` avec `sed` et
  les sorties Git sans NUL ne couvrent pas les noms arbitraires. Le produit
  conserve ses chemins réversibles et ses identités validées par Git.
- La copie `sites-wt-audit-msds.sh` dérive encore une destination partagée depuis
  son emplacement et exclut des dossiers par nom. Son archivage ne la rend pas
  directement exécutable depuis `scripts/session-msds/`, ni conforme à U2.
- `wt-dirty.py` ne contrôle pas tous les statuts, lit le porcelain de renommage
  comme une simple liste et compare surtout des blobs. Son résultat ne suffit
  pas à garantir une conservation de type, de mode, de lien ou d'index.
- Les retours Aristote et msds divergent sur ce que le bac à sable permet à Git.
  Cela décrit des contextes distincts, pas une règle générale. La forme d'une
  ligne de commande ne doit pas devenir une stratégie de contournement.
- Le retrait partiel après erreur de permission décrit dans le texte Aristote
  devient un scénario requis pour U6 : après chaque interruption, vérifier à la
  fois les chemins, l'administration et la restauration indépendante. Il n'a
  pas été reproduit ici et reste un fait rapporté.

La MR !7 rebasée fournit un autre scénario utile : même patch-id, nouveau HEAD,
ancienne copie locale et tests non rejoués. Le rapport doit distinguer ces quatre
observations. Une validation attachée à l'ancien SHA ne devient pas une validation
du nouveau ; « mergeable » n'est ni « tests réussis » ni « contenu intégré ».
Son état distant actuel n'a pas été vérifié dans cette tâche.

## Lots repris dans le MVP

1. **Rapport descriptif** : sujet borné, titre et lien de MR, identité d'association,
   cibles nommées, explication des inconnues, récap de cinq lignes et détail.
   Critères : les titres ne changent jamais le verdict ; les caractères de contrôle
   sont échappés ; homonymes et nouveau HEAD restent distingués.
2. **Inventaire local enrichi** : suivis, non-suivis, ignorés, index et métadonnées,
   sans exclusion automatique par nom. Critères : les trois mutations du témoin
   changent le manifeste de contenu ; un refus d'accès reste bloquant pour toute
   affirmation de conservation ; aucun filtre Git n'est exécuté.
3. **Identité d'exécution** : répertoire propre, provenance et résultats associés.
   Critères : deux exécutions concurrentes ne partagent ni fichier mutable, ni
   statut final ; aucun historique précédent n'est écrasé ou réinterprété.

Ces trois lots sont raccordés pendant la finition. Le [registre](finish-ledger.md)
donne les preuves exécutées et les limites. Une lecture Git d'index partagé a
modifié son mtime/ctime dans la fixture indépendante : la sémantique de ces index
reste donc explicitement inconnue, avec conservation de leurs octets bruts.
U5 à U7 restent conditionnées par [E1](decisions/exclusion-et-conservation.md), la
sauvegarde durable et une restauration indépendante. `apply` reste indisponible.

## Traçabilité des sources lues

Empreintes SHA-256 des sources locales, sans copier leurs données privées :

| Source relative au dossier historique | SHA-256 |
|---|---|
| `RETEX-session-msds-20261004.md` | `99fa9f557369ebdb54200a0e1dce4ff5a28b2bc1665b1a19cb6cf8f0b4532562` |
| `scripts/session-msds/wt-plan.sh` | `3a3422c34a9c6bb628105e9e19ccc94ca3d1d533389427f54575a1273bf13da3` |
| `scripts/session-msds/wt-apply.sh` | `b9e76bf3a14bdd192d7f0acd3ac1c630b065ccdbe79ceba6b6ef30a43daad313` |
| `scripts/session-msds/wt-describe.py` | `599fe4e30b12d7ab9bb8eb83c229a6988a8cd6b7829156c7efdb9cb5cabba2be` |
| `scripts/session-msds/wt-dirty.py` | `03109b5b99c064a9c876677f7044c9aaad940a382c1d7363f2bd2dc56499955b` |
| `scripts/session-msds/sites-wt-audit-msds.sh` | `066c91dc6e5457faa47f0596d50f7cc945e97eb4f94b482c9237ea8ae955cfa1` |

Les plans de 46, 38 et 50 lignes ont respectivement les empreintes
`88f93cf2cf8ec2fbea821e28133d3a1f4c2849d694ab2552ee3811c54e832e6d`,
`759075a1b423b8969c1edccc4a29ae0d9d5300ad3e9f0bf9d8c0b317b56b8371` et
`07a3daf4e2508266b42e58982b0bc9468acca642995e3e7449cd48eab4c391ee`.
