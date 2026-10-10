# Site de présentation de Worktree Sweep

Date : 9 octobre 2026. Statut : proposition à relire avant le plan d'implémentation.

[Index documentaire](../../README.md) · [Plan du produit](../../plan.md)

## Demande et version de référence

Créer le site de wt-sweep avec l'architecture Astro et le design system de la
landing Search Console MCP décrits dans le handoff du 9 octobre 2026. Adapter
la promesse, les exemples, les commandes et les parcours au produit réel.

Le checkout de cette conversation est sur `main`, à
`a551ae33d749c6ee947911a10926bb6decb46d26`. Il précède les fonctions livrées
de revue, mesure disque et rapport HTML. La référence produit retenue est
`c3e38f7810424ae57f58d6ea10c4db996e9122fb`, également présente dans la ref locale
`origin/main` et le worktree `html-reports`. Cet état local ne constitue pas
une nouvelle vérification de GitHub, de la CI ou de la publication de l'alpha.

L'implémentation du site devra partir de cette révision récente, dans une
branche isolée, sans déplacer les checkouts ni reprendre leurs modifications
concurrentes. Cette spécification ne change aucun comportement de la CLI.

## Public, promesse et action principale

Le site s'adresse aux développeurs sur macOS qui accumulent des worktrees,
notamment dans leurs workflows avec des agents. Ils veulent retrouver les
dossiers, comprendre leur taille et voir les éléments à conserver avant de
décider quoi en faire.

Promesse française : « Repérez les worktrees qui prennent de la place et le
travail à conserver. » La version anglaise conserve le sens du README :
« Find the worktrees taking up disk space and the work you need to keep. »

L'action principale est « Essayer sur une démo », vers un parcours d'installation
et un repo de test temporaire. L'action secondaire montre le rapport HTML. Le site permet
ensuite de choisir la CLI ou un assistant Claude/Codex pour examiner un repo
explicitement sélectionné. Il ne lance aucun scan depuis le navigateur.

## Composition de la landing

| Section | Contenu et destination |
| --- | --- |
| Header | Marque Worktree Sweep, Comprendre, Démarrer, Ressources, documentation, langue, repo et thème. Un même arbre alimente desktop et mobile. |
| Hero | Promesse, statut de revue en lecture seule, macOS, démo et aperçu d'un rapport synthétique. |
| Trois besoins | Retrouver les dossiers, repérer ceux qui prennent de la place, examiner ce qui reste localement. Liens vers les guides adaptés. |
| Rapport HTML | Vue par famille, recherche, verdicts, tailles, détails et copie de chemins. Captures claires/sombres et provenance du repo de test temporaire visibles. |
| Lire un verdict | `keep`, `verify`, `cleanup_candidate`, raison et prochaine vérification. Les lacunes restent présentes dans les exemples. |
| Fonctionnement | Choisir la portée et les refs, collecter, examiner le rapport, approfondir une ligne. Taille mesurée et espace récupérable restent distincts. |
| Démarrer | Deux entrées : compiler et essayer la CLI ; utiliser le prompt et la skill dans Claude/Codex. Les commandes gardent leurs prérequis. |
| Périmètre, FAQ et CTA final | Réponses sur macOS, données locales, Git, budgets, usage des assistants, licence et suppression indisponible. Footer documentaire et auteur. |

Les parcours partent des besoins du lecteur. Les quatre profils SEO du site
source ne sont pas repris. Les composants de parcours et de catalogue vérifiés
dans le handoff restent des motifs disponibles, sans imposer des pages inutiles.

## Architecture et design

Créer une application statique `site/` dans le repo wt-sweep. Reprendre Astro 5,
Starlight, TypeScript strict, CSS natif et les interactions minimales du site
source, avec un lockfile. Aucun frontend n'appelle la CLI ni ne reçoit de
chemins privés, identifiants forge ou inventaires réels.

Séparer les routes, `BaseLayout`, les sections, les contenus EN/FR, la navigation,
les styles, les interactions et la génération. Les versions et faits produit
viennent de `Cargo.toml` et d'un petit contrat explicite, validé au build. La
version du paquet n'est pas présentée comme une release publiée.

Conserver les fonds chauds, l'accent orange, les polices système, les titres
denses alignés à gauche, les séparateurs et les terminaux sombres. Le thème suit
la préférence mémorisée ou système avant le rendu. Les actions ont une cible
d'au moins 44 px, un focus visible et des retours de copie locaux. Les menus
gardent Escape, le retour du focus, le confinement mobile et les liens vers
les ancres de la landing depuis les pages secondaires.

Ne pas importer les commandes MCP, les outils SEO, les fournisseurs, les
métriques ni les illustrations métier Search Console. Réutiliser les captures
de rapports générés sur des repos de test de wt-sweep après vérification de leur provenance.

## Documentation et langues

La landing est disponible à `/` en anglais et `/fr/` en français. Le portail
Starlight fournit des pages correspondantes sous `/docs/` et `/fr/docs/`.

Le premier corpus porte sur l'installation, la démo, la référence CLI, les
tailles disque, le rapport HTML, les cibles et filtres, les assistants, les
contrats des résultats et les limites du produit. Ajouter un plan HTML du site
et un historique issu du changelog, sans qualifier `Unreleased` de publication.

Les sources canoniques conservent leur langue actuelle dans le repo. Les
traductions du portail sont des sources maintenues avec un manifest d'empreintes.
Un contrôle signale et bloque les traductions périmées lors de la validation.
Le build prépare une collection générée, séparée des originaux et ignorée par
Git. Une liste explicite autorise les pages publiques. Les plans internes,
validations privées, chemins personnels et résultats de scans réels en sont
exclus. Les liens Markdown sont convertis en routes connues ou font échouer
la préparation.

## Faits à préserver

- La CLI récente propose `scan`, `review`, `plan` et l'import forge hors ligne.
  `review --disk-usage --html --run-dir` est présent dans le code de référence.
- Les mesures logiques/allouées peuvent être partielles. Les limites APFS
  empêchent de les transformer en prévision d'espace récupérable.
- Les verdicts viennent de la CLI. Un assistant explique les rapports et
  propose des vérifications ; il n'établit pas les preuves à sa place.
- `cleanup_candidate` ne donne aucun droit de suppression. `apply`, la
  sauvegarde et la restauration restent indisponibles dans cette version.
- Les exemples de rapports proviennent de fixtures temporaires. Les résultats de
  tests datés ne deviennent ni une CI actuelle ni des métriques client.
- La licence observée est FSL-1.1-ALv2. Ne pas annoncer une licence MIT,
  une distribution open source, un paquet public ou un téléchargement validé
  sans vérifier l'état retenu avant publication.

## Livraison et critères d'acceptation

La première livraison est une version locale du site, avec captures et résultats
de validation. Le domaine, les réglages DNS et la publication distante restent
hors de cette étape. Le build public exigera une URL de site explicitement
configurée pour ses canonical, alternates, robots et sitemaps.

L'acceptation locale exige des tests de génération et de contenu, une vérification
Astro, un build, les liens et ancres construits, puis des tests navigateur :
navigation clavier/mobile, thème, copie, pages documentaires et changement de
langue. Contrôler le débordement à 390, 768, 1024 et 1440 px, les deux thèmes,
le mouvement réduit et Axe. Inspecter visuellement au moins desktop et mobile
dans les deux thèmes. Les captures historiques du site source ne remplacent
pas ces contrôles sur wt-sweep.

Vérifier canonical, alternates réciproques, métadonnées sociales, sitemap XML,
plan HTML et données structurées cohérentes avec la FAQ visible. Les données
structurées du produit décrivent une CLI macOS ; elles n'inventent ni note,
prix, économies ni gain mesuré.

Les tests du site utilisent des données synthétiques. Aucun scan de la machine,
fetch, prune, gc, nettoyage réel ou activation de skill global n'est nécessaire.
Rapporter séparément validation locale, CI et déploiement. Aucun commit, push
ou déploiement n'est effectué par cette spécification.
