# Changelog

## [Unreleased]

- Ajoute le rapport descriptif : sujet de commit borné, branche, HEAD complet,
  titres/liens de MR, observations et inconnues ; résumé de cinq lignes.
- Ajoute `scan --local` : manifestes bornés des fichiers suivis, non suivis et
  ignorés, index brut et reflog HEAD propre, empreintes et métadonnées par
  descripteurs. Les index partagés sont conservés bruts ; leur interprétation
  reste inconnue pour éviter l'effet de bord Git observé sur leurs dates.
- Ajoute `--run-dir` pour publier chaque exécution dans un nouveau dossier avec
  JSON, détail et reçu des empreintes. Les sorties restent hors des cibles ;
  aucune exécution concurrente ne remplace les fichiers de la précédente.
- Documente une troisième exception unsafe privée pour sélectionner le cwd Git
  par descripteur avant exec ; aucun retour au chemin après substitution.
- Éprouve les capacités E1 : lease d'écriture refusé et révocation de fichier
  régulier non prise en charge dans le contexte testé. U5 à U7 restent bloquées,
  avec la frontière d'autorité nécessaire décrite dans `docs/decisions/apply-readiness.md`.
- Refuse les flags réservés des index v3 et applique un même plafond aux entrées
  de fichiers, d'index et d'administration, après reproductions indépendantes.
- Arrête les recherches de relations quand le commit source est illisible,
  en conservant les associations forge. Le scénario FIFO retrouve son délai
  de test sans augmenter celui-ci. Le nettoyage des processus de test ne dépend
  plus d'une lecture globale de la table des processus.
- Analyse les retours de ménage msds et Aristote sans reprendre leurs inventaires
  partagés ni leurs opérations réelles. Documente huit pistes et trois prochains
  lots dans `docs/retex-sessions.md`.
- Ajoute cinq témoins sur dépôts jetables : ignorés retirés sans force, non-suivis
  refusés, empreinte de statut insensible au contenu, index perdu par un patch
  depuis HEAD et écrivain visible par descripteur mais extérieur au filtre cwd.
  Aucun moteur de retrait n'est activé.
- Termine les raccordements du MVP macOS U1 à U4 : montages natifs sans filtrage
  silencieux, matérialisation dataless désactivée et vérifiée pour le processus,
  ses threads et Git, lectures de métadonnées parallèles réglables de 1 à 8.
- Corrige l'identification du principal avec `--separate-git-dir`, conserve les
  incertitudes lors d'un changement de montage et refuse une sortie dans un
  dépôt encore non découvert par un inventaire global partiel.
- Ajoute les preuves natives, les positifs résidents avec includes, alternates
  et packs, les refus bornés des FIFO et le rejeu historique explicite de !91,
  !92 et de la MR !383 associée à `lead-app-mr43`. Aucun scan global réel exécuté.
- Documente les deux frontières FFI privées revues, seules exceptions locales
  à l'interdiction Rust du code unsafe. FileProvider réel et TCC restent non testés.
- Implémente l'inventaire U1 : familles Git exactes, chemins réversibles, OID
  complets, erreurs explicites, JSON/tableau et publications sans remplacement.
- Ajoute un exécuteur Git borné, environnement isolé, capture par sockets et
  contrôle croisé des administrations pour détecter les omissions de Git.
- Ajoute les oracles indépendants de conservation et les témoins d'E1 :
  exclusion des écrivains NON_DEMONTREE, `apply` toujours indisponible.
- Développe U2, U3 et U4 : découverte par racines, rapport de relations locales
  avec digest canonique, et ingestion d'exports forge liés à leurs reçus.
  Voir `docs/progress.md` pour les validations et critères ouverts.
- Vérifie les octets des objets Git contre leurs OID SHA-1/SHA-256, borne les
  lectures du rapport et refuse une sortie dans un worktree déplacé depuis
  l'inventaire. Les preuves historiques forge sont relues avant association.
- Signale les champs forge contradictoires sans association précise et conserve
  dans la couverture les entrées GraphQL dont le contenu manque.
- Ajoute les dépendances nécessaires aux formats JSON, empreintes et opérations
  sur descripteurs ; CI préparée pour récupérer puis tester hors ligne.
- Remplace le statut périmé du socle dans les instructions canoniques par un
  renvoi aux preuves d'implémentation, puis régénère les projections concernées.

- Exclut les handoffs locaux du suivi Git pour conserver les chemins privés hors des fichiers distribués.

- Initialise le crate `wt-sweep` sans dépendance, avec Rust épinglé et verrou Cargo.
- Ajoute aide/version et refus explicites des commandes produit non implémentées.
- Ajoute les tests CLI et ceux du générateur de configuration sur fixtures isolées.
- Génère les instructions et quatre rôles pour Claude et Codex depuis `harness/`,
  avec contrôle de dérive, refus des liens symboliques et des préimages modifiées.
- Ajoute la CI macOS et les plans de découverte, comparaison des alternatives,
  conservation et implémentation parallèle.
- Limites du socle initial : aucun inventaire, jugement Git, sauvegarde ni retrait implémenté.
  Présence des configurations distincte de leur chargement natif ; CI distante
  non exécutée lors de la préparation locale.
