# Site wt-sweep : réalisation locale

Spécification : [design validé](../specs/2026-10-09-wt-sweep-site-design.md). Base produit : c3e38f7810424ae57f58d6ea10c4db996e9122fb.

1. Générateur documentaire explicite : corpus EN/FR, hash des sources, refus des traductions périmées, liens internes et version Cargo. Test sur répertoire temporaire avant implémentation.
2. Landing bilingue : composants Astro, navigation partagée, thème, copie locale, captures synthétiques, pages nouveautés et plan du site. Réutiliser les contrats accessibles du site Search Console et les tokens BoldGuy.
3. Démo HTML native : fixture Git jetable uniquement, rapport et provenance, aucun parcours de la machine. Documentation des commandes, limites et installation depuis les sources.
4. Vérifications : tests générateur, Astro check/build, liens de toutes les pages, navigateur EN/FR, thèmes, menus/clavier/copie, Axe et largeur 390/768/1024/1440, lecture visuelle des captures. Contrôles Rust/Python/harness requis. Revue finale indépendante de l’ensemble.

## Interfaces

Le générateur produit src/content/docs depuis content/ et published-pages.json. Les chemins du portail sont /docs/<slug>/ et /fr/docs/<slug>/. Les routes marketing consomment les mêmes liens depuis src/data/content.ts. Les sources produit et les captures restent en lecture seule.

## Review Focus

Vérifier la licence FSL, absence de promesse de release ou d’espace récupérable, preuve limitée des captures, absence de données personnelles, liens EN/FR, état sans JavaScript, garde des sources périmées et domaines de production non inventés.

## Suivi

Ruling: livraison locale sans commit ni publication, conformément au périmètre validé ; les changements restent consultables dans le worktree.


Les quatre étapes sont réalisées. Le [registre de vérification](../../../site/VERIFICATION.md) consigne les résultats observés et les limites. Revue indépendante unique : deux constats importants reproduits puis corrigés, liens relatifs historiques et navigation mobile sans JavaScript ; libellés mobiles corrigés dans la même passe. Les tests complets du site sont verts après correction.

Ruling final : conserver ce worktree et le serveur de prévisualisation pour consultation, sans commit, push ou publication. Les étapes distantes restent hors de cette livraison locale.
