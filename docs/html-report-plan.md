# Rapport HTML par projet

[Index documentaire](README.md) · [Référence CLI](cli.md) · [Tutoriel](tutorial.fr.md)

`--html` produit `report.html` avec `scan`, `review` et `plan`. Le fichier est
autonome : styles et script sont embarqués, sans ressource réseau ni dépendance
à un LLM. Il s’ouvre directement dans le navigateur. Le sommaire et les preuves
restent lisibles sans JavaScript.

## Générer et ouvrir

Depuis le checkout compilé, renseignez le dépôt et sa cible confirmée :

```sh
(
  set -eu
  printf 'Chemin absolu du dépôt : '
  IFS= read -r repo_path
  printf 'Cible complète confirmée (refs/heads/... ou refs/remotes/...) : '
  IFS= read -r target_ref
  report_dir="$(mktemp -d /private/tmp/wt-sweep-html.XXXXXX)"
  ./target/release/wt-sweep review --repo "$repo_path" --target "$target_ref" \
    --summary --disk-usage --html --run-dir "$report_dir"
  open "$report_dir"/run-*/report.html
)
```

Le dossier de sortie doit être extérieur aux dépôts inspectés. `--html` exige
`--run-dir` ; il ne modifie ni le JSON émis sur stdout ni les verdicts.
`plan --inventory` accepte la même option, avec une cible ou une carte par famille.
`scan --html` fournit un inventaire marqué **not reviewed**, sans verdict inventé.

## Lire le bilan

- Le haut du rapport indique le périmètre, les dates d’observation, la couverture
  et les totaux source/sélection. Ce sont des observations passées, pas un état
  courant vérifié au moment de l’ouverture.
- Le sommaire regroupe les worktrees par famille Git, même si deux projets ont
  le même nom. Les filtres CLI et le regroupement demandé restent consignés.
- Chaque ligne conserve chemin, branche ou tête détachée, HEAD complet, verdict,
  raisons, contrôles manquants et tailles. Les détails sont repliables au clavier.
- La recherche, le filtre de verdict et le tri changent les lignes visibles dans
  le navigateur. Les totaux source et les mesures disque ne sont pas recalculés.
- Les octets logiques et alloués sont distincts. Une mesure absente ou partielle
  reste visible ; la somme des projets peut différer du total dédupliqué par inode.
  Aucun chiffre ne promet de l’espace récupérable sur APFS.

`cleanup_candidate` reprend la décision de la CLI. Il n’autorise aucune
suppression ; `apply` reste indisponible. Un reçu n’est pas une sauvegarde.

## Publication et limites

Le rendu est une fonction des observations déjà collectées, sans appel Git ni
nouvelle règle de décision. Les chemins conservent leur identité en octets ; les
balises, contrôles et noms hostiles sont affichés comme des données.

`run.json` lie la taille et l’empreinte SHA-256 de `report.html` comme celles du
texte et du JSON. Le plafond HTML est de 32 MiB. Un dépassement refuse la
publication avant de créer un run. Une erreur d’écriture/synchronisation HTML
ne publie pas de reçu réussi. Une erreur ultérieure de synchronisation du dossier
peut laisser un reçu visible sans établir la durabilité : conserver le code de
sortie et les diagnostics, pas seulement la présence de `run.json`.

Les tests de rendu, de publication et de CLI comparent les identités, verdicts,
empreintes et vues filtrées avec le JSON. Les vérifications de navigateur et le
SHA contrôlé sont consignés dans [le suivi de validation](progress.md).
