# Registre de finition de Worktree Sweep

[Index documentaire](README.md) · [Guide de démarrage](getting-started.md) · [Feuille de route](roadmap.md)

État courant : [triage T01-T09](#triage-t01-t09),
[livraison GitHub](#livraison-github-du-06102026) et [travail restant](roadmap.md).

Les sections précédant le checkpoint T01-T09 décrivent la clôture historique du
socle U1-U4 et des ajouts RETEX. Leurs chiffres, digest de binaire, absence de
remote et licence non renseignée sont l’état de cette passe, pas l’état courant.

Mandat historique du 04/10/2026 : terminer les lots issus du RETEX, éprouver les conditions
du nettoyage et préparer la livraison. Au démarrage, le dépôt n'avait ni premier
commit ni remote. La destination et la licence de publication ont été demandées
pendant le travail et restent sans réponse à cette clôture locale du 05/10.

## Lots livrés localement

1. Rapport descriptif : sujet borné du commit, titre et lien de MR, branche/HEAD,
   preuves et inconnues ; résumé de cinq lignes au maximum. Les descriptions
   n'entrent jamais dans le jugement d'intégration.
2. Inventaire `scan --local` : fichiers suivis, non suivis et ignorés, index,
   métadonnées et hash de contenu. Lecture par descripteurs, budgets explicites,
   erreurs conservées ; aucun certificat de sauvegarde ou d'immobilité.
3. Sorties `--run-dir` : dossier unique créé exclusivement hors des cibles,
   artefact JSON, détail texte et reçu portant l'identité de l'exécution.
4. Examen des mécanismes E1 et des alternatives sur sources primaires, avec
   sonde native exécutée. U5 et U6 gardent leurs préconditions.
5. Revue indépendante des lots réalisés, corrections et documentation.

## Responsabilités

L'intégrateur possède Cargo, lib/model/main/review/forge/output, le rapport et
la publication des exécutions. A possède local.rs, ses tests et les nouveaux
lecteurs Git. B écrit les oracles CLI indépendants dans test_enriched.py. C
examine E1 puis relit les changements finaux. Les modèles restent GPT-6.1 Sol
high pour A/B et GPT-6 Astra high pour C. Aucun dépôt réel n'est nettoyé.

## Décisions

- Les manifestes de fichiers sont demandés explicitement par `--local`, car le
  coût des lectures et empreintes dépend des fichiers présents. Un rapport sans
  manifeste conserve cette absence dans ses inconnues.
- `--run-dir` exige une base existante et crée un sous-dossier nouveau. Il est
  incompatible avec `--out`. La collecte échouée avant publication ne produit
  pas de reçu de succès ; un dossier partiel ne vaut pas une exécution réussie.
- Le contrat du nettoyage n'est pas affaibli pour obtenir un test positif.
  Les chemins manquants, les privilèges absents et les protections non démontrées
  doivent rester identifiables dans la livraison.
- L'oracle indépendant a observé que `ls-files --stage` rafraîchit mtime/ctime
  d'un index partagé malgré `GIT_OPTIONAL_LOCKS=0`. Le produit retire les lecteurs
  Git d'index : parser les versions ordinaires directement, conserver les octets
  partagés sans inventer leurs stages, et interroger les règles d'ignorés avec
  `check-ignore --no-index`. Une prélecture suivie de `ls-files` resterait exposée
  au remplacement concurrent de l'index.
- La revue a identifié la substitution du dossier de sortie entre création et
  acquisition du descripteur. Le contrôle porte désormais sur l'identité et le
  contenu du dossier acquis avant toute écriture. Il ne certifie pas l'exclusion
  d'un déplacement hostile après ce contrôle.
- Deux autres reproductions de revue ont échoué avant correction : flags
  réservés d'un index v3 présentés comme valides, et 25 203 enregistrements
  publiés depuis 12 600 fichiers malgré les plafonds annoncés. Le parseur refuse
  les flags inconnus ; un compteur partagé inclut index, fichiers et administration
  avant leur admission. Le positif v3 valide et les limites 1 à 4 passent.

## Conditions qui restent ouvertes

| Lot | Condition manquante |
|---|---|
| E1 | Autorité OS distincte, garde continu prouvé contre FD, mappings et opérations Git concurrents ; la sonde locale retourne EPERM/ENOTSUP. |
| U5 | E1 concluante puis conservation des ACL/xattrs/types spéciaux, objets et reflogs, persistance vérifiée et restauration sans source. |
| U6 | U5, plan distinct approuvé, retrait positif de fixture et reprise après interruptions. |
| U7 | U6, revue indépendante et sélection exacte autorisée pour une exécution réelle. |
| Publication | Destination distante et licence non renseignées ; CI seulement préparée. |

Les refus natifs ne prouvent pas qu'aucune architecture future ne peut satisfaire
E1. Ils empêchent de présenter ce binaire comme un outil de suppression sûr.
Voir la [décision et le protocole suivant](decisions/apply-readiness.md).

## Vérifications

Les tests descriptifs ont d'abord échoué sur les champs et options absents.
Les oracles vérifient ensuite les octets non UTF-8 du sujet, la troncature,
l'absence d'autorité des titres, les catégories de fichiers, les trois stages
de conflit, les hashes, les liens et les reçus de deux commandes concurrentes.
L'oracle de conservation indépendant compare aussi mtime/ctime ; l'atime reste
exclu comme dans le socle. Les changements à taille et statut identiques changent
le digest local. Les versions non prises en charge restent des lacunes explicites.

L'intégrateur a constaté RED puis GREEN pour la substitution de sortie. A a
reproduit RED puis GREEN pour les deux défauts du parseur et du budget signalés
par C. Le code et les tests de ces correctifs ont été relus par l'intégrateur.
La revue de C ne revendique ni instantané, ni exclusion E1.

La première suite complète a aussi révélé une régression du rapport : le scénario
FIFO d'alternates a pris 51,49 secondes, au-delà des 35 secondes du test. Deux
lectures de sujet avaient ajouté deux délais Git aux résolutions déjà vouées à
rester inconnues. Si le commit source n'est pas lisible, le rapport n'essaie plus
ces relations ; il garde les inconnues et associations forge. Le test passe en
21,497 secondes avec son délai inchangé. C a relu cette correction ; un sujet
malformé sur un objet valide continue de laisser le jugement Git fonctionner.

Le chemin d'erreur du test dépendait aussi de `ps`, refusé dans ce contexte.
B l'a remplacé par la récolte du seul processus créé et a vérifié ce nettoyage
sur un enfant réel, avec les ResourceWarning traités comme erreurs. Cette preuve
ne prétend pas connaître des descendants non observables.

État des vérifications à la clôture du 05/10/2026, après les correctifs :

| Contrôle | Résultat observé |
|---|---|
| Formatage Rust | PASS |
| Clippy, tous les targets, warnings refusés, hors ligne | PASS |
| Suite Rust complète | 92 passés, 2 ignorés : historique explicite et point d'entrée appelé par son test parent |
| Build debug et release, verrouillés, hors ligne | PASS |
| Suite Python complète | 83 recensés, 78 passés, 5 sautés : 4 replays historiques non demandés pour cette passe, 1 nom réel non UTF-8 refusé par le disque |
| Témoins E1 et RETEX | 6 passés le 04/10 ; ils démontrent des contre-exemples, pas l'exclusion |
| Parcours sur le binaire optimisé final | 5 passés : fichiers locaux, split index, résumé, reçus scan/plan/forge, FIFO alternates |
| Synchronisation du harness | 12 fichiers synchronisés ; activation native UNKNOWN |

Le binaire local est `target/release/wt-sweep`. SHA-256 des octets éprouvés :
`89d7517b024de1695bc333b52cac24718edd8c34bc371549aa470532b041ab4d`.
Il n'est ni installé globalement ni publié. La CI distante n'a pas été exécutée.

Avant le premier commit de livraison : 72 fichiers à suivre sur `main`, aucun
remote. Le mandat de finition couvre ce commit local ; il ne fournit ni
destination distante, ni licence, ni autorisation de nettoyage réel. Les fichiers
privés de reprise et les résultats de compilation restent exclus du suivi.


## Triage T01-T09

Validation locale du 05/10/2026 dans le worktree de triage, branche
`codex/worktree-triage`. Les résultats ci-dessous portent sur le raccordement
final de T01 à T09, après les corrections et la revue indépendante.

Le parcours `review` réutilise la découverte et le jugement de `plan`, avec cible
explicite, filtres, contexte et groupe. `--run-dir` conserve l’inventaire original,
la revue, le détail et le reçu. L’aide est propre à chaque commande. Les archives
`review/1` restent historiques et exigent un nouveau scan pour les contrôles C5.
Les trois verdicts sont `keep`, `cleanup_candidate` et `verify` ; l’usage des
candidats reste à confirmer. `apply` reste indisponible.

### Vérifications finales

| Contrôle | État final |
| --- | --- |
| Formatage, Clippy tous targets avec warnings refusés, build optimisé hors ligne | PASS |
| Suite Rust complète | 277 passés, 2 ignorés. |
| Suite Python complète | 253 recensés : 247 passés, 6 sautés, aucun échec ; Python 3.14, binaire optimisé figé. |
| Synchronisation du harness | PASS ; activation native `UNKNOWN`. |
| Tutoriel français sur le binaire final | Bloc shell exécuté sans modification : 3 worktrees, 2 `keep` et 1 `cleanup_candidate` avec cible ; sans cible, 2 `keep` et 1 `verify`. `apply` refuse avec le code 2, note et HEAD inchangés. |
| Documentation | 10 documents, 188 liens locaux et ancres, 18 blocs shell vérifiés. |
| Revue indépendante T09 | Conforme après correction, sans constat restant. |
| CI distante | Non vérifiée lors de cette validation locale ; consulter le statut du commit publié. |

SHA-256 du binaire final utilisé pour les tests Python, le tutoriel et l’essai
réel : `8cd44bb54abd32a061d886bb24f1e431633f4655b22655aa16031b8684ea4b4d`.
Les octets du binaire restent inchangés après chaque recette.

Les deux tests Rust ignorés sont un replay historique privé et un point d’entrée
enfant exécuté par son test parent. Les six tests Python sautés correspondent à
quatre replays historiques privés non demandés et deux chemins non UTF-8 que le
système de fichiers hôte refuse de créer. Les cas d’identité en octets restent
testés avec des enregistrements synthétiques.

La revue a aussi vérifié les identifiants Git en majuscules et les en-têtes de
commit malformés. Un test indépendant distingue l’épuisement du budget local
d’un worktree du délai maximal du moteur Git : le premier garde les observations
de la même famille disponibles ; le second arrête encore ses lectures dépendantes.
Le test échoue avec l’ancien comportement et passe après correction.

### Essai par dépôt

Le parcours final a examiné en lecture seule un dépôt choisi et ses worktrees
enregistrés, avec une référence locale confirmée avant exécution. Il a terminé
avec le code 0 en **197,276 secondes** :

| Observation | Résultat |
| --- | --- |
| Périmètre découvert | 1 famille, 48 worktrees, parcours achevé sans lacune de découverte. |
| Verdicts | 3 `keep`, 45 `verify`, 0 `cleanup_candidate`. |
| Conditions C1 | 45 worktrees passent les contrôles structurels ; ce résultat ne suffit pas au statut de candidat. |
| Relations Git | 1 commit identique, 3 relations d’ancêtre, 44 inconnues. |
| Résumés locaux | 4 worktrees examinés partiellement ; 44 non collectés après épuisement du budget global de revue locale. Aucun résumé n’est complet sur ses trois volets HEAD/index/disque. |
| Histoire privée | 112 racines couvertes, 209 inconnues. |
| Artefacts | Hashes et tailles des trois fichiers vérifiés contre le reçu ; liaison canonique entre inventaire et revue vérifiée. |
| Contrôles après exécution | Référence cible et liste des worktrees enregistrés inchangées. |

Les 48 worktrees gardent des inconnues. Le gros dépôt dépasse les budgets locaux
et d’historique ; cette version ne permet donc pas encore d’y désigner un candidat
au nettoyage. Un parcours de découverte terminé ne signifie pas que chaque
worktree a été entièrement analysé. Les métriques d’histoire ne mesurent pas le
temps de collecte locale ni celui de la vérification approfondie.

Les lectures Git groupées X02 restent une piste pour ce coût observé. Leur
condition d’ouverture sur la fixture de 100 worktrees n’a pas été rencontrée.
Aucune reprise persistante X01 ni prise en charge supplémentaire d’index X03
n’a été ajoutée : la découverte s’est achevée et l’index réel est un v2 pris en
charge. Les limites actuelles restent documentées dans la [référence CLI](cli.md).

Les commandes exactes, références, identifiants et rapports réels restent dans
les preuves privées, hors des exemples publics. Les guides utilisent des fixtures
synthétiques ou un chemin fourni par le lecteur. Aucun scan global réel ni
nettoyage n’a été exécuté. Cet essai ne prouve ni instantané machine, ni exclusion
d’écrivains, ni conservation exhaustive des fichiers de ce dépôt réel.

## Livraison GitHub du 06/10/2026

La [PR #1](https://github.com/FlorianBruniaux/wt-sweep/pull/1) a été fusionnée dans
`main`, commit `ddda77ddba24566328a83dad79b74d943cecd8e5`, tête de PR
`bfeb7967d4528fb204b885368240e227102c48c0`.

La [CI complète du commit 654413f](https://github.com/FlorianBruniaux/wt-sweep/actions/runs/37411615463)
a réussi : formatage, Clippy, tests Rust, build, 253 cas Python dont 247 passés
et 6 sautés, 6 témoins E1/RETEX, synchronisation des projections et diff final.
Le job de PR correspondant a terminé ses tests Python, puis a atteint sa limite
de 30 minutes avant les derniers contrôles. La tête fusionnée porte le délai
CI à 45 minutes ; son seul autre changement est le changelog. Le code produit
et les tests sont identiques à ceux de la CI complète réussie.

Lors du contrôle du 06/10 à 06:37 Europe/Paris, la
[CI du commit de fusion](https://github.com/FlorianBruniaux/wt-sweep/actions/runs/37414290125)
était encore en cours, après formatage et Clippy réussis. Ce relevé daté ne
remplace pas le statut courant du workflow. Aucun paquet ni binaire n'a été
publié. Les limites de l'essai réel et les conditions E1/U5-U7 restent ouvertes.
