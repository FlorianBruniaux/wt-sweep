# Première alpha de Worktree Sweep

[Index documentaire](README.md) · [Changelog](../CHANGELOG.md) · [Feuille de route](roadmap.md)

Version préparée : `0.1.0-alpha.1`. Tag prévu : `v0.1.0-alpha.1`, avec le statut
GitHub **Pre-release**. Cette préparation ne prouve pas qu’un tag ou une release
existe. Le statut de publication doit être vérifié sur GitHub.

## Périmètre de l’alpha

La CLI macOS inventorie les familles Git dans un périmètre choisi, mesure leur
occupation disque et produit une revue avec les verdicts `keep`, `verify` et
`cleanup_candidate`. Les rapports texte, JSON et HTML gardent les lacunes de
collecte. Le skill Claude/Codex et son prompt d’installation sont livrés avec
les sources.

La version reste en lecture seule. `apply` refuse les demandes. Une taille
allouée ne prédit pas les octets récupérables sur APFS ; un candidat ne prouve
pas l’absence d’utilisation par un agent ou un terminal. Les grandes familles
peuvent demander des reprises ciblées, même avec le budget `thorough`.

L’évaluation du collecteur natif [#20](https://github.com/FlorianBruniaux/wt-sweep/issues/20)
et la vision de gestion [#14](https://github.com/FlorianBruniaux/wt-sweep/issues/14)
restent séparées de cette publication.

## Conditions de publication

1. Fixer la licence avec le mainteneur. La FSL actuelle est une [licence Fair
   Source](https://fair.io/licenses/) ; une sortie open source immédiate demande un autre choix.
2. Vérifier le contenu suivi et son historique avant de rendre le repo public.
   Un détecteur de secrets ne remplace pas l’examen des résultats qu’il signale.
3. Publier le correctif de session Git, le prompt d’installation et les notes
   de version. Aligner Cargo, le changelog et le tag.
4. Exécuter les contrôles de [CONTRIBUTING.md](../CONTRIBUTING.md), puis obtenir
   une CI réussie sur le commit exact à taguer. Un job qui ne démarre pas ne
   valide aucun test.
5. Construire depuis un clone neuf, empaqueter le binaire et ses licences,
   vérifier l’archive extraite sur le repo de test temporaire, puis publier les sommes
   SHA-256 avec l’archive. Ne proposer que l’architecture effectivement testée.

## Archive macOS

La compilation utilise Rust 1.95, également déclaré comme minimum du paquet.
Les versions antérieures ne sont pas qualifiées pour cette alpha.

Le premier paquet testé localement est
`wt-sweep-v0.1.0-alpha.1-aarch64-apple-darwin.tar.gz`, pour Apple Silicon.
Il contient le binaire, sa licence, les notices des dépendances, le commit
source et une notice d’utilisation. `SHA256SUMS` accompagne le téléchargement.
Les skills restent disponibles dans le repo source.

Une archive locale testée ne prouve ni le téléchargement public ni son passage
dans Gatekeeper. La notarisation Apple et le support Intel/Linux ne sont pas
annoncés par cette préparation.

Après téléchargement des deux fichiers dans le même dossier, la vérification
et l’extraction prévues sont :

```sh
shasum -a 256 -c SHA256SUMS
tar -xzf wt-sweep-v0.1.0-alpha.1-aarch64-apple-darwin.tar.gz
cd wt-sweep-v0.1.0-alpha.1-aarch64-apple-darwin
./wt-sweep --version
./wt-sweep review --help
```

Ces commandes n’installent rien globalement. Utiliser ensuite le chemin absolu
du binaire à la place de `./target/release/wt-sweep` dans le
[tutoriel CLI](tutorial.fr.md).

## Vérification avant le tag

Le correctif de fermeture est testé en forçant la sortie du processus avant
la finalisation. Une sortie 0 doit conserver une session valide ; une sortie 7
doit produire `git_exit`. Avant correction, le nouveau test échoue avec
`input_io`. Les erreurs de protocole, dépassements et sorties en échec restent
des refus de preuve.

Les [résultats de validation locale](progress.md#préparation-de-la-première-alpha-09102026)
et la [recette de l’archive](progress.md#recette-de-larchive-apple-silicon)
sont consignés séparément de la CI. Le manifeste identifie le commit du paquet
candidat. Après le choix de licence et une CI réussie, reconstruire et vérifier
le paquet depuis le commit exact à taguer avant de publier.
