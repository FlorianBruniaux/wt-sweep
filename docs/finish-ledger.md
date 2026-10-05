# Finition de Worktree Sweep

[Index documentaire](README.md) · [Guide de démarrage](getting-started.md)

Mandat du 04/10/2026 : terminer les lots issus du RETEX, éprouver les conditions
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
