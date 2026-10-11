# Frontières natives macOS

[Index documentaire](README.md) · [Architecture](architecture.md) · [Contrats](contracts.md)

Le MVP utilise les exceptions unsafe privées `mounts::ffi`, `residency::ffi`
et `git::anchored_cwd`, ainsi que `disk::storage::native::ffi` pour les
statistiques de capacité demandées avec la mesure disque.
`libc` fournit les types de l'ABI et `getfsstat`. Les deux fonctions de politique
dataless absentes de cette version de libc sont déclarées avec les signatures
scalaires du SDK macOS. Aucun exécutable auxiliaire n'est requis par le produit.

Le lint du crate est passé de `unsafe_code = "forbid"` à `"deny"` pour autoriser
ces exceptions locales `#[allow(unsafe_code)]`. Chaque appel porte son
contrat `SAFETY` ; le reste du crate reste soumis au refus. Cette exception évite
de perdre les erreurs et montages non locaux dans une abstraction qui les filtre.
Chaque frontière a été relue par un autre agent que son auteur.

## Répertoire Git acquis par descripteur

L'inventaire local ouvre le worktree par descripteur sans suivre les liens.
Le fils Git sélectionne ce même répertoire avec `fchdir` avant exec. L'exception
`git::anchored_cwd` duplique le descripteur dans le parent avec CLOEXEC et le
garde vivant jusqu'au lancement. La fonction `pre_exec` ne fait que le syscall
async-signal-safe et la conversion de son errno ; aucune allocation, résolution
de chemin ou prise de verrou n'y est ajoutée. Une erreur refuse le lancement.

Sur ce macOS, utiliser `/dev/fd/N` comme cwd a retourné ENOTDIR. Le produit
n'utilise pas de repli vers le chemin initial, qui pourrait désigner une autre
identité après un renommage. Cette sélection ancrée n'empêche pas les écritures
concurrentes dans le répertoire ou son administration.

## Table des montages

`getfsstat(MNT_NOWAIT)` lit la table connue du noyau sans parcourir les volumes.
La mémoire est initialisée et bornée, les tailles vérifiées avant conversion,
les chaînes bornées au champ natif et les chemins conservés en octets.
Les erreurs ne deviennent jamais une table vide valide.

Deux tables consécutives, identiques et non saturées, sont requises. Des comptes
avant et après chaque lecture détectent les changements visibles et déclenchent
une nouvelle tentative bornée. Aucun filtrage silencieux des montages réseau,
cachés ou non navigables n'est appliqué. Source, type, flags et sous-type sont
conservés. La découverte compare aussi les tables avant et après le parcours.
Un volume disparu invalide la disponibilité des worktrees qu'il contenait ;
une table finale inaccessible empêche de conclure à leur absence.

`MNT_NOWAIT` peut retourner des informations conservées par le noyau. L'identité
opaque `fsid_t` n'est pas exposée par ce contrat : un changement ne touchant que
celle-ci peut échapper à la comparaison. Des tables égales ne prouvent ni une
photographie atomique, ni l'existence ou l'accessibilité de volumes non montés.
`available` signifie ici présent dans la table ; les refus d'accès du parcours
sont enregistrés séparément.

## Capacité sur descripteur

La mesure disque observe au plus une fois chaque device depuis un répertoire
déjà acquis et vérifié. `fstatfs` utilise le type Darwin de `libc`, une mémoire
initialisée et un fd emprunté vivant pendant l'appel. Le code de retour est
contrôlé avant lecture. Les noms sont bornés aux tableaux natifs et conservés
en octets ; une chaîne sans NUL provoque une erreur, jamais une troncature.
Les identités device/inode avant et après doivent égaler l'ancre attendue.

Les produits blocs × taille de bloc sont vérifiés séparément. Capacité nulle,
compteurs indéfinis ou débordements restent inconnus. Un disponible ou libre
supérieur à la capacité n'est pas publié comme valide. Ces observations ne
modifient ni la table des montages ni la preuve de nettoyage.

Le plafond est de 4 096 tentatives de devices dans le budget disque restant.
Un échec n'entraîne pas de retry ; les octets par device sont conservés.
Les contrôles de temps précèdent et suivent chaque lecture d'identité et de
statistiques. Une limite atteinte arrête les appels suivants et laisse les
capacités inconnues. Ils ne garantissent pas l'interruption
immédiate d'un syscall bloqué dans le noyau. Aucun montage supplémentaire n'est
ouvert, aucune commande externe ou modification système n'est nécessaire.
La frontière exige une revue indépendante avant livraison.

## Refus de matérialisation

Avant toute lecture produit et avant de lancer Git, `setiopolicy_np` fixe la
politique `IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES` à `OFF` pour le processus
et le thread courant. `getiopolicy_np` doit confirmer ces deux valeurs. Tout
échec, valeur inconnue ou plateforme non prise en charge provoque un refus.
Les threads de lecture appliquent aussi cette garde.

La politique est propre au processus. Elle ne modifie ni configuration globale,
ni permissions, ni hooks. Elle est héritée par les nouveaux processus, ce qui
couvre les chemins que Git atteint indirectement : includes de configuration,
alternates, objets compressés et pointeurs d'administration. Le diagnostic
`SF_DATALESS` refuse aussi un candidat identifié avant lecture de son contenu.

Les tests natifs démarrent dans un sous-processus avec la politique active,
vérifient son passage à OFF puis son héritage après exec. Les tests injectés
couvrent les erreurs de réglage, de lecture et de confirmation. Les fixtures
réelles résidentes couvrent les includes, alternates, packs et administrations
séparées ; les FIFO démontrent un refus ou un délai Git borné. Un délai Git ne
prouve pas que le fichier n'a jamais été ouvert.

Un véritable placeholder FileProvider et les permissions TCC ne sont pas testés.
La politique empêche la matérialisation dataless, pas tout accès réseau sur un
volume déjà monté. Elle ne démontre ni exclusion des processus d’écriture, ni résidence
atomique, ni interruption immédiate d'une opération bloquée dans le noyau.

## Sources et preuves

Les déclarations ont été vérifiées dans le SDK et les manuels macOS installés :
`getfsstat(2)` et `setiopolicy_np(3)`. Références Apple :
[getfsstat](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/getfsstat.2.html),
[setiopolicy_np](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/setiopolicy_np.3.html),
[TN3150](https://developer.apple.com/documentation/technotes/tn3150-getting-ready-for-data-less-files).
Le [registre](progress.md) donne les résultats effectivement observés.
