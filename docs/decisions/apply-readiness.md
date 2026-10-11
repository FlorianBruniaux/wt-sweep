# Accès à U5/U6 : décision E1 du 04/10/2026

[Index des décisions](README.md) · [Architecture](../architecture.md)

L’[évaluation du 11 octobre](../research/writer-exclusion-evaluation-2026-10-11.md)
rejoue ces témoins et ajoute mappings, alias et interruptions. Elle conclut
l’essai #29 sans établir E1 ; les observations du 04/10 ci-dessous restent datées.

**NON_DEMONTREE. Aucun mécanisme d'exclusion conforme n'a été établi avec les
droits et contraintes de cette session. U5 reste bloquée, puis U6 et U7 par
dépendance.** Cette conclusion porte sur les mécanismes examinés et les preuves
disponibles ; elle ne démontre pas l'impossibilité d'un mécanisme futur.

Le [plan](../plan.md) exige une exclusion continue depuis le début de la capture
jusqu'au retrait et aux vérifications finales. Elle couvre les fichiers, les
administrations Git et les objets, y compris les processus d’écriture non coopératifs,
descripteurs déjà ouverts et commits concurrents. Une sélection approuvée ne
remplace pas cette propriété.

Cette exploration garde le produit en lecture seule. Aucun changement de droits,
flags de fichiers, configuration globale, montage, extension système ou processus
tiers n'a été effectué. L'UID effectif de la sonde était 501, sur macOS 26.5.1
arm64. La sonde n'établit pas les droits d'un autre exécutable ou environnement.

## Preuves locales et mécanismes écartés

Les [témoins E1 existants](exclusion-et-conservation.md) ont déjà démontré une
écriture par descripteur depuis un cwd extérieur après renommage et verrou
coopératif, une mutation à taille/mtime identiques et un commit détaché tardif.
Ils ne sont pas présentés comme nouvellement exécutés dans cette exploration.

La nouvelle sonde est rejouable depuis la racine du repo :

```sh
python3 -I -S -B experiments/exclusion/probe_native_capabilities.py
```

Elle crée un seul fichier régulier dans un dossier temporaire exclusivement
possédé sous `/private/tmp`, appelle les API sur ce fichier, puis retire cette
fixture. Résultat observé, sortie 0 :

| Appel réel | Résultat |
|---|---|
| `fcntl(fd, F_SETLEASE, F_WRLCK)` | Refus `EPERM` (1) |
| `fcntl(fd, F_GETLEASE)` | `F_UNLCK` (2), aucun lease acquis |
| `revoke(fichier_régulier)` | Refus `ENOTSUP` (45) |
| Écriture via le FD préexistant après ce refus | 6 octets écrits, relus identiques |

Ces mesures valident des refus de capacité et un FD toujours utilisable en écriture.
Elles ne valident ni exclusion globale, ni sauvegarde, ni restauration.

| Mécanisme | Confrontation au contrat |
|---|---|
| `flock`, `fcntl`/OFD, `O_EXLOCK`, verrou entre instances | Les manuels locaux `flock(2)` et `fcntl(2)` décrivent des verrous coopératifs. Ils ne forcent pas un autre processus d’écriture à les prendre ; le témoin FD a déjà franchi cette frontière. |
| Renommage, quarantaine, `lsof`, seconde empreinte | L'observation et le changement de nom n'invalident pas un FD ouvert et n'interdisent pas une écriture suivante. Ils restent utiles pour détecter une dérive, sans fermer l'intervalle de capture/retrait. |
| Modes, ACL, `UF_IMMUTABLE` | Aucun changement testé. Le propriétaire peut retirer `UF_IMMUTABLE` ; ces contrôles ne constituent pas une autorité distincte face au même UID. `SF_IMMUTABLE` exige le superutilisateur. Voir [Apple chflags(2)](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/chflags.2.html) et le manuel local. |
| `revoke` | Le manuel réserve la révocation aux fichiers spéciaux bloc/caractère. Le refus mesuré sur fichier régulier est conforme à cette limite. Voir [Apple revoke(2)](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/revoke.2.html). |
| Leases Darwin | Le SDK expose les constantes, mais [XNU `vnode_setlease`](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/vfs/vfs_subr.c) exige l'entitlement privé `com.apple.private.vfs.file-leases`. Le code prévoit aussi la rupture des leases après délai. Le refus local ne fournit aucun garde exploitable. La lecture de XNU n'est pas une validation de tout comportement du noyau installé. |
| Snapshot ou montage en lecture seule | Aucun essai. Un snapshot conserve un état sans exclure les écritures dans l'original. Le manuel local `mount(2)` interdit les écritures sur un montage `MNT_RDONLY`, y compris celles nécessaires au retrait ; repasser en écriture demande une autre frontière d'exclusion. Les opérations de montage sont hors autorisation actuelle. |
| Endpoint Security | La création d'un client exige un entitlement et une autorisation TCC selon le SDK `EndpointSecurity/ESClient.h`. Aucun client n'a été installé ni lancé. Couvrir ouvertures, mappings existants, mutations de noms et reprise après panne reste à démontrer. Voir [Apple System Extensions](https://developer.apple.com/system-extensions/). |
| Garde de résidence du MVP | `IOPOL_MATERIALIZE_DATALESS_FILES_OFF` empêche la matérialisation dans les processus protégés. Il ne retire aucun droit d'écriture à un autre processus et ne satisfait pas E1. |

## Comparaison des références

Cette comparaison complète [les références](../references.md). Elle repose sur
la documentation primaire consultée, sans installation ni exécution des moteurs
de nettoyage. Les liens de code aux révisions précises déjà consignées dans les
références n'ont pas pu être rechargés par l'outil web durant cette passe ; leur
audit antérieur n'est pas présenté comme une nouvelle lecture.

[AgentRinse, contrat de sécurité](https://github.com/vincentkoc/agentrinse/blob/main/docs/safety.md)
décrit une revalidation, une quarantaine par renommage, une ref de récupération
et une purge distincte. Il refuse notamment les worktrees détachés et le contenu
ignoré dans cette voie. Son contrat exclut explicitement un processus hostile
du même utilisateur et renvoie ce problème à une isolation OS extérieure.
Son verrou global sérialise ses propres applications. Ce périmètre déclaré ne
démontre donc pas notre E1 pour un arbre arbitraire encore accessible aux autres
processus d’écriture. Ce constat ne prétend pas avoir reproduit un défaut de son programme.

[Ward](https://github.com/michaelmillar/ward) décrit une vérification du bundle
par clonage et une archive complémentaire pour les hooks, configurations et
non-suivis. Ce contrôle inspire une restauration indépendante ; il ne fournit
pas, à lui seul, une preuve d'exclusion des processus d’écriture ou de conservation de toutes
les catégories exigées par U5. L'exclusion des ignorés relevée dans l'audit
antérieur reste référencée dans `docs/references.md`, sans nouvelle validation
comportementale ici.

[Stalewood](https://github.com/retif/stalewood) documente une découverte qui saute
`node_modules` et `.git`, une intégration reconnue dans toute autre branche et
un retrait par Git. Ces choix diffèrent de notre couverture et de nos cibles
d'intégration explicites. Ils ne lèvent pas le verrou E1/U5. Les autres produits
du catalogue ne sont pas réaudités dans cette passe bornée.

## Plus petite frontière testable sans réduire E1

La condition manquante est une autorité d'écriture exclusive, vérifiable et
maintenue sur le périmètre entier. Elle doit soit empêcher l'accès des autres
processus d’écriture dès la création du stockage, soit acquérir l'exclusion de manière à
traiter aussi leurs références déjà ouvertes. Une affirmation « processus
arrêtés », une identité de PID ou la connaissance de nos seuls enfants ne prouve
pas cette condition pour un repo utilisateur existant.

Le prochain essai utile porterait sur **un seul repo synthétique dans un domaine
de stockage placé sous une autorité OS distincte de l'UID adversaire**. Un
superviseur doit contrôler tous les accès au stockage avant son exposition. Une
VM avec dossier hôte partagé ou image modifiable par ce même UID ne remplit pas
automatiquement cette condition. Cette architecture est une candidate à éprouver,
pas un mécanisme déjà disponible ou certifié dans cette session.

La preuve minimale à apporter à cette frontière est séquentielle :

1. Acquérir un garde lié aux identités exactes de tous les chemins et objets
   concernés, ou refuser avant capture. Prouver le traitement des FD et mappings
   existants, nouveaux opens, alias physiques, administrations et objets Git.
2. Faire tenter les écritures par des enfants possédés du même UID que les
   processus d’écriture ordinaires : FD depuis cwd extérieur, mapping, création/renommage,
   mutation à taille/mtime identiques et commit tardif. Synchroniser par messages.
   Vérifier le refus effectif ou l'inclusion exacte de l'état admis par le protocole.
3. Interrompre le détenteur du garde à chaque transition. Une perte de contrôle
   doit interdire tout retrait ; aucun lease expirant ni déverrouillage implicite
   ne peut laisser la suppression continuer.
4. Sous ce garde seulement, livrer U5 : toutes les catégories restaurables ou
   refusées, barrière de persistance vérifiée, restauration dans une destination
   neuve sans source ni alternates. U6 doit ensuite réussir un retrait positif
   sur fixture et les refus adversariaux, avec un plan distinct approuvé.

Le responsable du prochain lot doit d'abord fournir cette frontière d'autorité
et son protocole d'acquisition. Les droits, l'isolation et leur preuve manquent
actuellement ; demander uniquement un accord pour `apply` ne les crée pas.
Une sauvegarde diagnostique peut être étudiée séparément, mais ne compte pas comme
U5 et n'autorise aucun retrait. `apply` reste indisponible.
