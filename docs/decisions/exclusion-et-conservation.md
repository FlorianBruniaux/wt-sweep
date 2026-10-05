# E1 : exclusion et conservation

[Index des décisions](README.md) · [Architecture](../architecture.md)

Décision du 04/10/2026 : **NON_DEMONTREE**. Les témoins exécutés sur
macOS 26.5.1 arm64 réfutent les mécanismes particuliers ci-dessous. Ils ne
démontrent pas l'impossibilité de tout mécanisme futur. `apply` reste indisponible ;
U5 demeure bloquée. E1 ne bloque pas le rapport en lecture seule U1 à U4.

## Rejouer les témoins

Depuis la racine du dépôt, sans dépendance Python externe :

```sh
python3 -B -m unittest discover -s experiments/exclusion -p 'test_*.py' -v
python3 -B experiments/exclusion/experiment.py
```

Le programme possède exclusivement un dossier `wt-sweep-e1-*` créé par
`TemporaryDirectory` sous `/private/tmp`. Il retire seulement cette fixture à la
fin. Il ne reçoit aucun chemin de dépôt réel. Git utilise un environnement
remplacé, un HOME vide et un répertoire de templates/hooks vide ; aucun réseau,
configuration globale, permission, hook installé, fetch, prune ou gc. Les commits
créés sont ceux du dépôt jetable. Les enfants sont attendus et arrêtés en cas
d'erreur. Les échanges READY/GO/DONE ordonnent les écritures ; les délais bornent
les attentes sans produire de course fondée sur `sleep`.

## Résultats observés

| Témoin | Observation et limite |
|---|---|
| Descripteur ouvert depuis un cwd extérieur | Le parent détient un verrou `flock` exclusif sur le fichier. L'enfant ouvre ce fichier avant renommage du dossier. Après renommage, sa tentative de prendre le même verrou est refusée, puis son écriture directe réussit. Renommage et verrou coopératif ne ferment pas ce chemin d'écriture. |
| Même taille et mtime | Une seconde écriture change le SHA-256 des octets tout en restaurant exactement `st_mtime_ns` et en gardant la taille. Le test compare réellement les deux lectures. Il ne prétend pas que ctime est inchangé. |
| Commit détaché tardif | Après capture du HEAD détaché, un enfant produit un nouveau commit lisible avec son blob. HEAD change et la branche sentinelle reste identique. La capture initiale n'inclut pas cet état tardif. Aucune sauvegarde n'est simulée. |
| Vue partielle des processus | Un observateur injecté retourne une liste vide pendant que le protocole prouve l'existence et l'écriture du processus enfant. C'est une injection de visibilité partielle, pas un test de permissions TCC ni une mesure de couverture de `lsof`. La visibilité réelle complète reste `UNKNOWN`. |

Le test initial échouait avec `UNKNOWN != NON_DEMONTREE` avant implémentation.
La suite finale passe : un test de comportement vérifie les trois témoins réels,
la vue partielle injectée et les inconnues de conservation. Ce succès valide les
contre-exemples, pas l'exclusion. Un mécanisme limité aux écrivains coopératifs
doit refuser toute situation où cette coopération n'est pas établie.

Les [retours de session](../retex-sessions.md) ajoutent cinq tests : retrait
sans force qui efface les ignorés, témoin positif de refus des non-suivis,
empreinte HEAD/status insensible aux octets, perte de l'index distinct dans un
patch depuis HEAD et observation native `lsof` d'un écrivain au cwd extérieur.
Les six tests de cette suite passent localement. L'observation `lsof` porte
uniquement sur le processus enfant possédé ; aucune visibilité machine complète
n'est revendiquée. Ces résultats renforcent les refus, sans lever E1.

## Contrat nécessaire à une restauration future

Le périmètre à préserver inclut tous les octets suivis, non suivis, ignorés et
masqués, les noms exacts, types et topologie, les index complets et leurs étapes,
HEAD détaché ou symbolique, refs propres, reflogs, administrations commune et
propres aux worktrees, ainsi que tous les objets nécessaires. Les alternates,
sous-modules, dépôts imbriqués, éléments inaccessibles et liens hors périmètre
exigent une prise en charge explicite ou un refus. Cette expérience n'en fait pas
l'inventaire exhaustif et ne fournit aucune sauvegarde admissible pour un retrait.

| Catégorie | Couverture locale | Restauration |
|---|---|---|
| Octets, noms, modes, propriétaires, dates, flags | Octets et `mtime_ns` éprouvés ; mode ordinaire lu, sans changement de permissions. Autres propriétés non testées. | `UNKNOWN`, bloquante |
| Liens symboliques | Cible relative lue sans la suivre. | `UNKNOWN`, bloquante |
| Liens physiques | Deux noms du même inode vérifiés, compteur observé à 2. Liens externes non explorés. | `UNKNOWN`, bloquante |
| ACL | Aucun transport ni restitution implémenté ; aucune ACL ajoutée. | `UNKNOWN`, bloquante |
| Attributs étendus et forks | Python local ne fournit pas `os.listxattr` (`AttributeError`). Aucune absence d'attribut déduite. | `UNKNOWN`, bloquante |
| Types spéciaux | FIFO créé et identifié par `lstat`, sans lecture bloquante. Sockets, périphériques et autres types non éprouvés. | `UNKNOWN`, refus requis |
| Objets et métadonnées Git | Nouveau commit détaché et blob relus dans la fixture source. Pas de restauration indépendante. | `UNKNOWN`, bloquante |

Même une copie correcte de ces catégories ne ferme pas la fenêtre d'écriture.
L'exclusion doit couvrir capture, publication durable, retrait et vérifications
finales, sur les fichiers comme sur toutes les administrations et objets concernés.

## Persistance

Avant tout retrait futur, données, objets, manifeste, reçu, noms publiés,
répertoires et journal doivent franchir une barrière de persistance documentée
pour le système de fichiers retenu. Il faut traiter les erreurs de synchronisation
et d'espace, puis vérifier la restauration dans une destination neuve sans source
ni alternates. L'appel `fsync` du témoin FD synchronise une écriture particulière ;
il ne valide pas cette barrière complète.

Le type du volume temporaire, les garanties APFS, la perte d'alimentation, les
caches matériels et la récupération après perte d'écritures ne sont pas testés :
`UNKNOWN`. Aucun second scan, digest, renommage atomique, journal ou accord humain
ne transforme ces inconnues en preuve. Une sauvegarde éventuelle resterait
diagnostique tant qu'E1 et les garanties de conservation U5 ne sont pas établies.
