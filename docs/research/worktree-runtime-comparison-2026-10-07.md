# Sweep, Holt et AgentRinse : comparaison contrôlée

Comparaison du 07/10/2026, sur des dépôts Git synthétiques jetables exclusivement. Conserver Sweep pour l'audit fondé sur une cible choisie. Holt et AgentRinse apportent des vues et des protocoles de récupération intéressants, mais les verdicts observés ne satisfont pas le contrat de preuve de Sweep. Aucun retrait, déplacement, restauration, hook, configuration globale ou dépôt personnel n'a été exécuté.

Les trois lecteurs ont été exécutés : 48 audits retenus, avec un positif intégré non vide et les mêmes OID source/cible par scénario. Sweep distingue le reflog privé, l’objet manquant et la cible choisie dans les cas où les verdicts externes diffèrent. Le stash partagé reste une nuance : Sweep inventorie le stash dans le stockage commun, sans protéger automatiquement le worktree lié propre.

## Versions, installation et périmètre

| Outil | Révision exacte | Exécution et configuration |
| --- | --- | --- |
| Holt 0.4.7 | `b99e098e214f4e84388ffa9d6fa6b675f951732a` | CLI native ESM, `status --cwd REPO --base refs/heads/main --json --no-symbols --include-primary`, avec puis sans `--strict-read-only`. Aucun backend sémantique installé. |
| AgentRinse 0.8.2 | `6d5e047e00ef0193f6b75b952060a80ea310b1a4` | CLI native compilée, `audit --home HOME --config CONFIG --json --no-state`. Tous les adaptateurs désactivés sauf Git, racine explicite d'une fixture. `worktrees.minAgeMinutes=0`, sans changer son code. |
| Sweep | SHA-256 `a622edf51eb32ef8fe58991703f9b404bbc67d1526bddaec9f4950206d6f26bf` | `review --repo REPO --target FULL_REF --format json --run-dir OUT`. |

L’intégrateur a fourni la release Sweep après intégration HTML/formatage. Elle a été copiée en `/private/tmp/wt-sweep-comparison-1ffk8k26/sweep-release-final` et son empreinte a été comparée à la source avant exécution. Après l’optimisation de sérialisation et le correctif P2 HTML, les douze audits Sweep ont été refaits sur un corpus neuf. Les classifications, motifs de conservation et conditions manquantes des deux worktrees concordent dans 12 cas sur 12 avec le premier binaire `3f7a8b6cee43806a3899aae6589c7bf7a5d16bd49ba7609af0e8f2bb41fa6406`. Les douze commandes finales terminent avec code 0 ; l’oracle ne constate aucune écriture dans les sources lisibles ou HOME. Les 36 observations des concurrents sont conservées. Les temps restent diagnostiques sous la charge de la suite Python de l’intégrateur.

La source part du commit `14f00ea454795fd6df1f86dec72ec6573d3e546e` avec changements locaux : la preuve de cette comparaison porte sur le binaire nommé par son SHA-256, pas sur une release publiée.

Les clones inspectés restent intacts sous `/Users/florianbruniaux/Sites/divers-tests/wt-sweep-comparison-20261007/`. Leurs HEAD ont été revérifiés avant copie. L'installation utilise les copies `/private/tmp/wt-sweep-comparison-1ffk8k26/build/{holt,agentrinse}`, Node `24.16.0`, npm `11.13.0`, TypeScript `7.0.2`, HOME et cache dédiés. Les scripts des manifests ont été inspectés. Aucun script de cycle de vie npm n'a été exécuté.

Commandes d'installation dans ces copies, avec `HOME`, `XDG_CACHE_HOME` et `npm_config_cache` dirigés vers le préfixe temporaire :

```sh
# Holt, verrou fourni conservé, dépendances facultatives et de développement omises.
npm ci --ignore-scripts --omit=optional --omit=dev --no-audit --no-fund
# AgentRinse, versions directes exactes du manifest ; résolution transitive npm.
npm install --ignore-scripts --package-lock=false --no-audit --no-fund --omit=optional
npm install --no-save --ignore-scripts --package-lock=false --no-audit --no-fund --omit=optional @typescript/typescript-darwin-arm64@7.0.2 @koromix/koffi-darwin-arm64@3.1.6
node node_modules/typescript/bin/tsc -p tsconfig.build.json
```

Le premier fetch en sandbox échouait avec `ENOTFOUND registry.npmjs.org`. Les retries autorisés hors sandbox ont installé 101 packages pour Holt et 56 pour AgentRinse. Le build AgentRinse initial refusait le binaire ARM facultatif de TypeScript ; l'ajout exact ci-dessus a permis la compilation sans modifier le source. Son verrou pnpm n'a pas été appliqué par npm : le pin du source et des dépendances directes est établi, la reproduction exacte des transitives nécessite de conserver le graphe installé ou de refaire l'essai avec pnpm. Le graphe installé est conservé dans les logs temporaires.

Les fixtures sont créées avec `/usr/bin/git`, version `2.50.1 (Apple Git-155)`. Les concurrents choisissent `/opt/homebrew/bin/git`, version `2.56.0`, via le PATH explicite. Les lecteurs reçoivent une allowlist d'environnement : aucun secret hérité, configuration Git globale/système désactivée, téléchargements implicites et transports interdits, `GIT_OPTIONAL_LOCKS=0`. Ce contrôle externe fait partie de l'expérience ; il ne certifie pas la neutralisation native d'AgentRinse.

Le cwd du lecteur est un dossier neutre appartenant au run. Le premier diagnostic AgentRinse depuis le checkout voyait son propre cwd dans `lsof`, retourné avec code 1 et des enregistrements non vides, puis protégeait tout l'audit comme incomplet. Depuis le cwd neutre, les positifs donnent `processOwnership=idle`, `inspectionComplete=true` et `eligible/certain`. Aucun mock, backend de processus remplacé ou patch concurrent n'a été utilisé.

Le seuil d'âge AgentRinse est ramené à zéro pour éprouver ses autres critères sur des fixtures neuves. Son seuil par défaut de 14 jours aurait ajouté un veto d'âge. Une URL distante `https://example.invalid/synthetic.git` et des refs distantes locales synthétiques sont configurées ; aucune communication distante n'a eu lieu et la présence actuelle d'un objet sur un serveur reste inconnue.

## Corpus et oracle

Le [runner](../../scripts/compare-worktree-tools.py) crée un nouveau run exclusif sous `/private/tmp`. Chaque outil et chaque mode reçoivent leur propre corpus, généré avec les mêmes octets et dates Git fixes. Une famille indépendante par scénario évite qu'un stash partagé ou un objet absent contamine le témoin positif. Les identités inode et chemins diffèrent volontairement entre fixtures ; elles ne sont pas comparées entre clones.

Onze scénarios donnent douze audits par mode : le changement de cible comprend une observation avant et une après. Les commandes, codes, sorties natives, raisons, OID et manifests sont conservés dans les [preuves synthétiques](../../tests/data/comparison/competitor-evidence-2026-10-07.json), avec `$RUN`, `$BUILD` et `$NODE` remplaçant uniquement les préfixes d'exécution. Les [douze audits Sweep](../../tests/data/comparison/sweep-evidence-2026-10-07.json) conservent la release, les verdicts et le même oracle. Les empreintes des octets stdout/stderr originaux sont enregistrées avant normalisation des préfixes. Ces copies normalisées ne sont pas des reçus canoniques à revalider ; les artefacts originaux restent dans les runs temporaires.

L'oracle indépendant des produits parcourt les octets et modes de tous les fichiers de la fixture, y compris l'index, HEAD, refs, reflogs et objets Git. Il préserve les liens, répertoires, types spéciaux et erreurs de lecture. Il compare avant/après et liste les ajouts, suppressions et modifications. Il contrôle séparément le HOME du lecteur. Il ne passe pas par Git, les statuts des produits ou leurs fingerprints.

L'oracle n'est pas une sauvegarde : il ne vérifie pas ACL, xattrs, durabilité, FileProvider, TCC, exclusion des écrivains, restauration ni tous les montages. Le contenu du dossier `denied` est volontairement inaccessible ; son erreur `EACCES` est conservée avant et après. Son absence de delta ne prouve pas la conservation d'octets qu'il n'a pas lus. Les permissions sont rétablies seulement après les observations sur cette fixture exclusivement, pour laisser le run inspectable.

## Faits observés

Les valeurs Holt ci-dessous viennent de `safe[]`, pas d'un simple compteur. `safe=true` est une affirmation externe de reproductibilité selon son contrat, pas une autorisation de suppression Sweep. Le mode strict estampille les comparaisons commités `approximate` ; le mode normal est `measured`.

| Fixture | Sweep release | Holt strict / normal | AgentRinse stateless | Portée de la preuve |
| --- | --- | --- | --- | --- |
| Propre, HEAD intégré dans `main` | `cleanup_candidate`, C1-C5 satisfaites | `safe=true` | `eligible`, `certain` | Positif non vide pour les trois lecteurs. Le checkout principal reste protégé. |
| Commit courant absent de `main`, couvert par `origin/topic` local | `keep`, `head_not_integrated` | `safe=false`, fichier absent de la base | `eligible`, `certain` | AgentRinse observe `remoteReachable=true` par refs locales. Cela ne prouve pas l'intégration dans `main`. |
| Index `staged\n`, disque `disk\n`, HEAD `base\n` | `keep`, `local_staged`, `local_unstaged` | `safe=false`, modification locale | `protected`, `dirty-worktree`, staged=1 et modified=1 | Les deux couches sont conservées par l'oracle. Aucun chemin de récupération testé. |
| Non suivi et ignoré avec octets propres | `keep`, `local_untracked`, `local_ignored` | `safe=false`, ignoré non vérifiable | `protected`, `dirty-worktree`, `ignored-worktree-content` | Motifs positifs visibles, aucune exclusion implicite de l'ignoré. |
| Stash partagé avec index, disque et non suivi propres au stash | `cleanup_candidate`, stash partagé inventorié | Worktree `safe=true`, stash total=1 et atRisk=1 séparément | Worktree `eligible` ; pas de finding stash | Les deux lecteurs qui inspectent le stash le publient séparément. Le stockage commun demeure présent ; aucune restauration indépendante n’a été éprouvée. |
| Commit abandonné seulement dans les reflogs, worktree encore sur `topic` | `keep`, `private_history_not_preserved`, C4 manquante | `safe=true` | `eligible`, `certain` | L'oracle conserve les reflogs et l'OID privé. Le veto d'état détaché ne masque pas ce cas. |
| Blob de HEAD absent du stockage Git | `verify`, C2/C3/C5 manquantes | `safe=true` | `eligible`, `inspectionComplete=true` | Les fichiers restent présents sur disque ; aucun verdict externe ne démontre ici la reproductibilité de leur objet manquant. |
| Cible choisie absente | `verify`, C2/C5 manquantes et erreur de résolution | Code 1, base non résolue | `eligible` | AgentRinse n'accepte pas de cible d'intégration équivalente ; ce n'est pas une erreur de son contrat. |
| Dossier local illisible | `verify`, résumé local incomplet, C2/C3/C5 manquantes | `safe=true`, sans skipped | `unknown`, inspection/mesure/processus incomplets | Holt ne donne pas de protection dans ce cas. L'oracle a une lacune explicite `work/denied`. |
| `main` déplacée vers l'ancêtre après le premier audit | `cleanup_candidate` puis `keep`, `head_not_integrated` | `safe=true` puis `safe=false` | `eligible` puis `eligible` | Deux captures distinctes, pas une simulation de race. `origin/main` reste identique : les politiques diffèrent. |
| Deux versions commitées en conflit | `keep`, `head_not_integrated` | `safe=false`, contenu manquant | `protected`, `unpushed-commit` | Positif de rétention commité ; le mode normal Holt écrit des objets de fusion. |

Ces observations réfutent l'équivalence des verdicts sur ce corpus. Elles ne prouvent pas une perte de données, puisque aucune action de retrait ou de quarantaine n'a été exécutée. La qualification `safe` ou `eligible` ne peut pas remplacer C1-C5, un contrôle d'usage ou les gates E1/U5-U7.

## Effets d'écriture et temps

Sur les 48 audits retenus, aucune modification ou suppression de fichier préexistant, index, ref ou reflog n'a été détectée dans les périmètres lisibles. Sweep, Holt strict et AgentRinse `--no-state` n’ont ajouté aucun objet ni fichier de HOME. Les résultats de l'audit sont écrits par le runner dans un dossier extérieur aux dépôts.

Holt normal a ajouté un objet loose `commit 204` dans le scénario de cible modifiée (`613a4ef43e1f7710b13ef3fe4b68d10a156baef7`), puis un arbre et un blob dans la fixture de conflit. Les manifests distinguent ces ajouts des modifications d'état conservé. Le mode normal n'est donc pas sans écriture Git. Aucun cache utilisateur écrit n'a été observé dans les HOME isolés.

Les temps sont ceux d'un seul passage exploratoire sous charge concurrente, avec caches OS non contrôlés. Holt omet les symboles ; AgentRinse mesure les octets et les processus, Sweep collecte un autre contrat de preuve. Aucun classement de vitesse, premier fait utile ou équivalence de travail n'est établi. Les valeurs par commande restent dans le JSON pour reproduire et borner l'observation, pas pour conclure sur une performance comparative.

## Décisions par capacité

| Réf. | Capacité | Décision | Motif et suite bornée |
| --- | --- | --- | --- |
| CAP-1 | Audit et verdict fondé sur une cible choisie, local et histoire privée | Conserver et développer Sweep | Sweep confirme ses verdicts sur le positif et les cas de cible, d’objet absent et de reflog. Ne pas traduire automatiquement `safe`/`eligible` en candidat Sweep. |
| CAP-2 | Détail de stash et relation de contenu entre worktrees | Adapter une vue de faits, si besoin confirmé | Holt détecte le stash à risque séparément. Un import doit garder outil, SHA, argv, date et statut, sans changer C1-C5. Pas de nouveau moteur d'audit concurrent dans le produit. |
| CAP-3 | Rapport lisible, graphes et sorties progressives | Retenir les idées de présentation | L'essai porte sur JSON natif, pas sur le rendu HTML ni l'accessibilité. Développer la projection Sweep à partir de ses propres contrats. Aucun code Holt copié. |
| CAP-4 | Gestion quotidienne des environnements et ADE | Arrêter de dupliquer cette capacité dans Sweep | Hors périmètre de l'audit. Worktrunk et Superset restent des références à éprouver séparément ; aucune adoption runtime démontrée ici. |
| CAP-5 | Quarantaine, reprise, restauration autonome et retrait | Différer l'adoption et toute écriture Sweep | Les scénarios de récupération ne sont pas exécutés. Comparaison D5 future avec writer-exclusion, durable-backup et independent-restore ; `apply` reste indisponible. |
| CAP-6 | Intégration directe Holt ou AgentRinse comme autorité de nettoyage | Ne pas adopter | Les raisons natives sont du contexte inspectable. Leurs contrats observés ne remplacent pas l'intégration dans la cible ni la conservation privée. |

Il n'est pas justifié d'abandonner le moteur Sweep sur la base de ces deux lecteurs. Il est justifié de limiter son développement aux capacités de preuve et de restitution qui manquent aux sorties observées. Les protocoles de récupération externes restent des candidats à comparer, sans preuve runtime de restauration dans cette tâche.

## Reproduction et vérifications

```sh
python3 -B scripts/comparison/test_harness.py
python3 -B scripts/compare-worktree-tools.py --holt BUILD/holt/bin/holt.mjs --agentrinse BUILD/agentrinse/dist/cli.js
python3 -B scripts/compare-worktree-tools.py --holt BUILD/holt/bin/holt.mjs --holt-normal
# Release copiée et identifiée par SHA-256 :
python3 -B scripts/compare-worktree-tools.py --sweep SWEEP_RELEASE
```

Le runner refuse un lancement sans binaire explicite et n'accepte aucune racine réelle à auditer. Ses seules commandes produit sont `status`, `audit --no-state` et `review`. Les refs et octets synthétiques se créent dans un répertoire exclusivement possédé. Les chemins de sortie, cwd, HOME, config et codes réels sont consignés.

Les deux tests comportementaux ont été observés rouges avant implémentation puis verts : mutation des octets à taille constante, index/refs/reflog/objets et obligations des fixtures positives/négatives. Les corrections de conflit et de reflog encore branché ont leurs propres échecs observés avant correction. Le premier replay Sweep atteignait le rapport puis refusait la publication avec `unsafe_output (write_new): No such file or directory` : le harnais lui passait une base inexistante. Le passage à la base de sortie déjà créée a donné un positif `cleanup_candidate`, puis 12 codes de sortie 0 sur le corpus. Ce diagnostic de harnais est exclu des audits retenus. Dernier passage : deux tests réussis. La suite complète est réservée à l'intégrateur et n'a pas été lancée par ce lot.

Les logs complets restent sous `/private/tmp/wt-sweep-comparison-1ffk8k26/logs`. Les runs retenus : `wbhvn7o9`, `njhhnnpv`, `wse7vvwo`, `lp1vfpwd`, `sd0jsopk`, `3nm5yitb`, tous préfixés `/private/tmp/wt-sweep-comparison-run-`. Sweep : `0cd6l6b5` (release finale), `kernd3db` (release précédente). Les premiers diagnostics depuis un cwd Git et le premier témoin reflog détaché sont exclus des 36 audits concurrents retenus ; ils ne servent pas à décider les verdicts finaux. Le contrôle statique confirme 48 observations, OID équivalents entre outils, zéro modification ou suppression lisible préexistante, cinq liens locaux valides et des empreintes sources distinctes des copies normalisées.

Les [analyses statiques](../worktree-source-analysis-2026-10-07.md) et [l'annexe backend](worktree-source-2026-10-07/backend.md) expliquent les contrats lus. Les preuves ci-dessus ne valident pas les licences, le comportement des versions futures, un serveur distant, la production, la restauration ou l'absence d'écrivains.
