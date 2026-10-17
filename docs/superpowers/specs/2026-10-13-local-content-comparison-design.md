# Comparer le contenu local avant une revue de retrait

Statut : spécification approuvée dans la conversation du 13 octobre 2026.
Le [plan d'implémentation v2](../plans/2026-10-13-local-content-comparison-v2.md)
a été approuvé et implémenté dans le checkout local.
La [recette](../../research/local-comparison-acceptance-2026-10-13.md) distingue
les contrôles observés, la revue indépendante et les limites de livraison.

## Résultat attendu

Pour chaque worktree examiné, Sweep indique quels changements locaux ont un
contenu déjà présent dans les références choisies, quels contenus diffèrent et
quelles comparaisons restent impossibles. Le rapport répond à « Que reste-t-il
à récupérer ? » avec les chemins, les preuves et les limites de la collecte.

Le programme peut constater une égalité de contenu. Il ne peut pas déterminer
si un développement différent est utile, obsolète ou une erreur. Ces décisions
restent humaines. Un fichier différent est donc à examiner ; son âge et la date
de fusion ne permettent pas de le déclarer jetable.

Le lot garde `triage/1`, les verdicts `keep`, `verify`, `cleanup_candidate` et
leurs conditions C1-C5. Un fichier ignoré identique ailleurs reste une raison
de conservation dans ce contrat. Le nouveau bilan explique cette raison ; il
ne transforme pas une observation de doublon en autorisation de supprimer.
`apply` reste indisponible.

## État de départ lors de la conception

[`LocalSummary`](../../../src/model.rs) sépare index, disque, conflits, fichiers
non suivis et ignorés. Le collecteur de
[`src/local/summary.rs`](../../../src/local/summary.rs) lit déjà les contenus
avec des plafonds et vérifie les objets Git. Ses échantillons ne contiennent
qu'une partie des chemins observés : ils ne peuvent pas servir de liste complète
des fichiers à comparer.

[`LocalInventory`](../../../src/model.rs) conserve des empreintes et des
métadonnées lors de `scan --local`. Un ancien manifeste décrit son intervalle
de capture ; le nouveau bilan doit collecter ses preuves courantes dans le run
de `review` ou `plan`, sans certifier l'état actuel à partir de ce manifeste.

[`src/review.rs`](../../../src/review.rs) conserve aujourd'hui les fichiers
locaux observés et demande cinq compteurs complets à zéro pour C3. La comparaison
proposée utilise les lectures existantes, mais conserve ses propres faits et sa
couverture. Les [contrats](../../contracts.md#triage-policy) restent la référence
pour le verdict.

## Périmètre et références

L'analyse est facultative dans `review` et `plan`. Les options sont
`--compare-local` et `--compare-reference ABSOLUTE_PATH`.
La seconde exige la première. L'option existante `--worktree` limite la collecte
aux chemins enregistrés choisis ; les filtres d'affichage restent indépendants.

Les références Git sont exclusivement les cibles d'intégration explicites de
la famille. Sweep capture et vérifie leurs OID complets avant la comparaison,
puis revalide les refs en fin de collecte. Il ne devine pas une branche cible,
ne fait pas de fetch et n'utilise pas l'ensemble des anciens blobs comme preuve
que le contenu actuel est intégré.

Le checkout de référence est facultatif, explicitement fourni et présent dans
la même famille Git. Pour plusieurs familles, la CLI refuse un checkout unique
ambigu ; l'utilisateur peut examiner les familles séparément. Elle refuse de
comparer un worktree à lui-même. Elle vérifie les identités du checkout fourni
et ajoute ses frontières à l'interdiction d'écrire les rapports dans les cibles.
Sans cible Git ni checkout de référence explicite, la demande est refusée avec
une explication des références nécessaires. Une cible demandée mais illisible
reste une référence inconnue ; elle ne disparaît pas de la liste de comparaison.

Les chemins sont comparés au même emplacement relatif. Une égalité trouvée sous
un autre nom n'est pas recherchée dans ce lot. Un checkout de référence sale
peut fournir une observation « mêmes octets présents ailleurs », mais cette
observation ne prouve ni l'intégration Git ni une sauvegarde durable.

## Observations et règles déterministes

Chaque observation distingue sa couche : index, fichier sur disque, suppression
locale ou fichier supplémentaire. Sweep compare l'index et le disque séparément.
Un index différent ne disparaît pas du bilan parce que le disque est identique
à une cible. Les chemins en conflit restent à examiner, avec leurs stages.

| Résultat proposé | Preuve nécessaire | Sens pour le lecteur |
| --- | --- | --- |
| `matches_target` | Même contenu vérifié, type et mode Git au même chemin dans un arbre cible nommé | Ce contenu est présent dans la version de la cible capturée. |
| `matches_reference` | Même contenu, type et mode POSIX au même chemin dans le checkout fourni, lectures stables | Ce contenu est également présent dans cette copie locale. |
| `different` | Références demandées entièrement examinées, aucune égalité trouvée | Le contenu ou le mode diffère dans les références examinées. |
| `unknown` | Objet absent, lecture refusée, conflit, type non pris en charge, changement ou plafond atteint | Sweep n'a pas terminé cette comparaison. |

Une preuve positive d'égalité survit à l'échec d'une autre référence : la ligne
nomme celle qui correspond et garde la lacune restante. En revanche, Sweep ne
publie `different` que si toutes les références demandées pour cette ligne ont
été examinées. Une absence locale inaccessible n'est pas une suppression connue.

Pour une suppression locale, l'absence au même chemin dans l'arbre cible peut
correspondre à la suppression, après lecture complète de l'arbre pertinent.
Une suppression ne correspond pas à un fichier vide. Les changements de mode,
les liens symboliques et les types d'entrée font partie de la comparaison.

Les objets Git sont lus avec le transport contrôlé et vérifiés contre leur OID.
Les fichiers locaux sont ouverts via les frontières natives existantes, avec
contrôles d'identité avant et après lecture. Les liens sont comparés par leurs
octets de destination sans suivre leur cible. Gitlinks, repos imbriqués,
fichiers spéciaux et contenus dataless produisent une lacune explicite pour ce
lot ; ils ne peuvent pas contribuer à un bilan complet sans travail à examiner.

Les filtres Git, fins de ligne et normalisations ne sont pas exécutés. Une
différence de représentation conserve une différence ou une lacune. Aucune
normalisation JSON/TOML ne permet de conclure à une égalité en effaçant des ports,
chemins, versions ou valeurs de configuration. Les ACL, attributs étendus et
identités propriétaire/groupe restent hors de l'égalité de contenu et de mode
annoncée ; le bilan ne prétend pas comparer tout le contrat de restauration.

Les noms `node_modules`, `.next` ou `.env` ne constituent aucune preuve. Le
parcours garde fichiers ignorés, cachés et non suivis. Les dépendances et caches
peuvent être annotés comme emplacements habituels de génération, avec ce statut
descriptif visible ; ils ne sont ni exclus ni automatiquement déclarés jetables.
Le rapport ne contient pas d'octets de fichiers, de valeurs de configuration ou
de secrets, seulement chemins, types, modes et empreintes.

## Collecte, couverture et coût

Une projection facultative du collecteur local conserve les observations de
comparaison au moment où les contenus sont lus. Elle ne reconstitue pas une
liste exhaustive à partir des huit échantillons de `LocalCount`. Les lectures
d'objets vérifiés et les arbres cibles sont réutilisés au sein d'une famille,
dans le même run et sous les plafonds existants.

L'analyse utilise les profils `standard|thorough` et leurs plafonds locaux de
temps, octets, objets, chemins et profondeur. Les lectures du checkout de
référence sont facturées dans ce même budget ; elles ne doublent pas gratuitement
le travail autorisé. Ajouter un plafond de 4 096 observations détaillées en
standard et 32 768 en thorough. Ces valeurs sont des allowances proposées,
pas des garanties de débit ou de complétude.

Quand un plafond de détails est atteint, les compteurs conservés restent des
observations positives, mais le bilan global devient incomplet. Chaque arrêt
garde sa cause et le périmètre non comparé. Une collecte non demandée est distincte
d'un budget épuisé. Une modification de fichier, d'index, d'identité ou de ref
invalide les preuves concernées ; elle ne devient jamais une égalité réutilisée
sur cache. Les intervalles restent visibles et ne constituent pas un instantané.

Le bilan compte les chemins distincts et les observations par couche séparément.
Il ne double pas le nombre de fichiers lorsqu'un chemin a un index et un disque
différents. Pour les quatre compteurs de chemins, une couche `different` classe
le chemin dans les différences ; sinon une couche `unknown` le classe dans les
inconnues ; sinon toutes les couches doivent correspondre à une cible Git pour
le compter dans les cibles. Les autres chemins entièrement comparés sont comptés
dans les copies locales. Les compteurs de couches gardent les inconnues même
quand une différence positive a déjà classé le chemin ; la couverture globale
reste alors partielle. Un bilan « tous les changements locaux comparés sont déjà présents
ailleurs » exige une couverture complète et une égalité établie pour chaque
couche pertinente. Une liste tronquée ou un conflit empêche ce bilan.

## Artefact et compatibilité

Créer un artefact facultatif `local-comparison.json`, au schéma proposé
`wt-sweep.local-comparison/1`. Les noms de ce paragraphe sont un contrat de design
à implémenter, pas des champs déjà disponibles.

L'artefact porte son empreinte canonique, l'empreinte de l'inventaire et celle de
la revue source `review/2`, les intervalles, les références demandées et capturées,
les identités physiques, les OID complets, les limites appliquées et la couverture.
Chaque ligne contient le `BytePath` réversible du worktree, le chemin relatif,
sa couche, son résultat, les preuves positives et les lacunes. Les références
Git et filesystem portent des provenances distinctes.

Garder `review/2` et `triage/1` inchangés. Le reçu `run.json` inscrit le nouvel
artefact, sa taille et son SHA-256, après écriture et synchronisation. Le
publisher accepte uniquement ce nom supplémentaire contrôlé, impose les mêmes
frontières et plafonds, et refuse la publication si l'artefact ne peut pas être
publié correctement. Une erreur ne peut pas laisser un rapport complet annonçant
une comparaison absente. Les sorties mono-fichier JSON restent des revues ;
demander la comparaison exige un run permettant de sauver son artefact lié.

Les rendus ne joignent un bilan qu'après vérification de son digest, de sa liaison
à la revue et de ses chemins. Un bilan ancien ou incohérent est refusé, jamais
réattaché par simple égalité du nom de branche. Un ancien run reste lisible avec
le libellé « Comparaison du contenu local non collectée ». Les filtres gardent
les totaux source et la couverture d'origine.

## Rapport humain et compétence assistant

La carte de chaque worktree et le récapitulatif montrent « Que reste-t-il à
récupérer ? », avec les nombres de chemins présents dans une cible, présents
dans la copie locale, différents et non comparés. Les détails donnent la
référence exacte et la couche du changement. Une égalité partielle ne masque
pas un index différent, un conflit ou les conditions C1-C5 encore manquantes.

Le prochain geste dépend des faits : examiner les chemins différents, relancer
un worktree précis quand une comparaison est incomplète, ou terminer la revue
Git et de conservation lorsque tous les changements examinés ont un équivalent.
Le mot « inédit » ne signifie pas « utile » et l'absence de différence locale
ne signifie pas que les commits et l'historique privé sont conservés.

Le HTML fonctionne hors ligne et affiche les faits sans JavaScript ; la console
et le texte utilisent les mêmes compteurs et références que le JSON. Une PR/MR
importée peut donner le contexte de fusion, mais son état ne remplace aucune
preuve de contenu. Aucune estimation de place récupérable n'est ajoutée.

Modifier la compétence canonique dans `harness/skills/wt-sweep`, puis générer les
projections. Elle vérifie le nouvel artefact et explique ses résultats sans
reclasser le verdict CLI, lire les secrets ou proposer un retrait exécutable.
Elle garde l'autorisation individuelle de l'utilisateur distincte des faits.

## Fichiers et responsabilités

L'intégrateur possède les contrats, le raccordement CLI et la publication.
La conception prévoit un module de comparaison dédié pour ses types et son
jugement pur, raccordé à `src/local/summary.rs`, `src/review.rs`, `src/main.rs`,
`src/run.rs` et aux garde-fous de `src/output.rs`. Les rendus console/texte et HTML
consomment l'artefact lié. Aucune nouvelle dépendance n'est justifiée à ce stade.

Mettre à jour les contrats, le guide CLI, le guide HTML, la compétence canonique
et le changelog lors de l'implémentation. Le checkout contient déjà des changements
disque/HTML et des tests locaux : les conserver et établir des préimages avant
les nouvelles modifications. Une délégation exige une autorisation applicable
et une responsabilité par fichier ; cette spécification ne lance aucun agent.

## Acceptation

Les preuves Git utilisent des repos temporaires possédés ; les contenus et les
configurations de test sont synthétiques. Chaque positif a un contre-exemple.

1. Modification locale reproduite exactement dans la cible : égalité prouvée
   avec chemin, mode et OID ; modification d'un octet : `different`.
2. Index inédit avec disque égal à la cible : les deux observations restent
   visibles et le bilan ne conclut pas à l'absence de travail différent.
3. Fusion squash avec arbres équivalents : le bilan explique les contenus ;
   aucune équivalence d'arbre ne devient une ascendance ou ne satisfait C2.
4. PR mergée avec commit supplémentaire ou modification locale : le statut
   forge n'efface pas le travail observé ni l'historique privé à vérifier.
5. Fichier ignoré identique dans la copie explicite, puis fichier ignoré unique
   sous `node_modules` : le premier nomme sa copie ; le second reste à examiner.
6. Configuration différant seulement par une version de Node ou un port :
   différence conservée ; aucune valeur synthétique secrète dans les rapports.
7. Suppression déjà représentée dans la cible, suppression face à fichier vide,
   changement de bit exécutable et cible de symlink différente : résultats distincts.
8. SHA-1 et SHA-256, objet corrompu/manquant, chemins non UTF-8 et noms contenant
   des caractères HTML : preuves exactes, lacunes et échappement conservés.
9. Fichier ou ref changeant pendant la lecture, index intent-to-add, conflit,
   gitlink, repo imbriqué et entrée dataless : aucun faux bilan complet.
10. Plusieurs cibles, une égalité positive et une cible illisible : preuve
    positive retenue et couverture partielle ; aucune différence globale inventée.
11. Plafonds de temps, octets et détails : compteurs partiels, chemins omis et
    ciblage d'une reprise lisibles ; un zéro partiel n'est pas une absence.
12. Artefact altéré, mauvais digest de revue, mauvais chemin, erreur d'écriture
    ou de synchronisation : liaison/publication refusée ; ancien run lisible.
13. Oracle indépendant avant/après pour les fixtures : aucun contenu, index,
    ref, configuration, hook ou objet cible modifié. `apply` reste un refus.
14. Revue avec/sans comparaison sur fixtures stables : mêmes verdicts C1-C5,
    mêmes inventaires et mêmes faits locaux préexistants ; nouveau bilan non vide
    sur un positif et différent sur son contre-exemple.

Exécuter les contrôles obligatoires de `AGENTS.md` à l'implémentation, avec tests
de comportement ciblés avant les changements, puis la suite intégrée. Cette
spécification n'annonce aucun test produit réussi ni gain de performance mesuré.

## Suite autorisée

Le plan v2 et son implémentation locale ont été approuvés dans la conversation.
Les commits, une publication et tout retrait réel gardent leurs autorisations
distinctes. Les règles ci-dessus ne lèvent pas E1 ni les conditions U5-U7.
