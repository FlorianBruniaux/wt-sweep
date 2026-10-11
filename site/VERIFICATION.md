# Vérifications du site wt-sweep, 09/10/2026

État courant : le déploiement du commit `02dd1099` a réussi le 11/10/2026.
Les pages publiques rapport et changelog EN/FR contiennent le dashboard attendu.
Les observations du 9 et du 10 octobre ci-dessous restent historiques ; le
[registre de livraison](../docs/progress.md#dashboard-et-ci-automatique-11102026)
porte les preuves distantes et les résultats des suites.

Livraison locale du 9 octobre dans le worktree `wt-sweep-site`, base produit `c3e38f7810424ae57f58d6ea10c4db996e9122fb`. Les entrées conservent l’état de livraison au moment des contrôles décrits. À cette date, le site n’était pas encore déployé.

## Référence relue

Le handoff Search Console et ses fichiers référencés ont été lus. Les parcours par profil, le catalogue, AuditCoverage, la navigation et la découverte documentaire sont maintenant intégrés dans le site source. Les 15 fichiers qui portent ces fonctions sont identiques à la copie isolée précédemment vérifiée ; HEAD source observé à la dernière lecture : `1fe2d5f7a31bbf093ca05cda812a260a99045a1e`. Les contrôles de cette copie ont passé : Astro, build et tests des sources, du HTML et du navigateur. Le worktree source concurrent est resté en lecture seule.

Dans wt-sweep, les entrées suivent les questions sur l’espace disque, la conservation et les preuves. Le portail présente les commandes CLI et leurs contrats. Les profils SEO et outils du serveur Search Console ne figurent pas dans les contenus wt-sweep.

## Résultats observés

- Astro : 38 fichiers contrôlés, aucune erreur, aucun avertissement ; 31 pages construites.
- Site : 6 tests du générateur, du HTML et de l’origine HTTPS de test ; 24 tests navigateur réussis. EN/FR, clair/sombre, 390/768/1024/1440 px, Axe, clavier, copie, navigation sans JavaScript, recherche documentaire EN/FR et filtres du rapport.
- Produit : formatage Rust, Clippy et tests Cargo réussis en mode verrouillé et hors ligne. Python 3.14 : 305 tests réussis, 6 ignorés par le harnais. Expériences d’exclusion : 6 tests réussis. Synchronisation du harnais : 22 fichiers conformes.
- Démo : vrais repos Git temporaires de test uniquement, 3 verdicts keep et 1 cleanup_candidate ; conservation de la fixture et empreintes des artefacts vérifiées.
- Revue indépendante : liens relatifs historiques et navigation mobile sans JavaScript corrigés après reproduction. Les libellés mobiles restent sur une ligne. Tests de régression et lecture des captures corrigées réussis.

## Limites conservées

La prévisualisation locale interdit l’indexation. Une compilation avec une origine HTTPS réservée vérifie les métadonnées et le sitemap sans publier. Domaine public, disponibilité distante du repo, release publiée et activation native des skills ne sont pas validés. Les captures datées gardent leur provenance distincte. La taille mesurée ne représente pas un espace récupérable garanti. `apply` reste indisponible.


## Variante Compact, 10/10/2026

Référence lue sans modification : `compact-site-header/google-search-console-mcp/site`, HEAD `c1cbc47ba8386c58f6dd806be5d3dcf8e851a2c7`, avec ses changements locaux d’en-tête.

Adoption : logo GitHub avec nom accessible, globe FR/EN, icônes lune/soleil sans cadre et bouton Install/Installer orange en dernière position. L’alignement conserve le conteneur du hero wt-sweep. Sur mobile, Docs et Installer occupent chacun une ligne complète ; la langue, GitHub et le thème partagent une rangée intermédiaire. L’accès sans JavaScript reste disponible.

Le seuil mobile de wt-sweep reste 1024 px ; les libellés plus courts permettent ce seuil, validé aux largeurs 390/768/1024/1440 px. Les contrôles gardent des cibles de 44 px et le cycle de focus du menu se termine sur Installer.

Preuves : test Compact observé en échec avant modification, puis 4 cas Compact EN/FR et clair/sombre réussis ; suite complète de 28 tests navigateur réussie, dont Axe et navigation sans JavaScript. Les 6 tests documentaires, de démo et de compilation à origine HTTPS réservée sont réussis. Astro : 38 fichiers, aucune erreur ni avertissement ; 31 pages construites. Six captures FR contrôlées : ordinateur, mobile et menu mobile dans les deux thèmes. Aucun débordement ; les transitions sont désactivées en mouvement réduit.

Fichiers modifiés : `src/components/SiteHeader.astro`, `src/styles/global.css`, `src/scripts/interactions.ts`, `src/data/content.ts`, `tests/browser/site.spec.ts`, les deux pages de changelog et leur manifeste ; changelog du repo et présent registre. Les contrôles Rust/Python du 09/10 ci-dessus restent historiques et n’ont pas été rejoués pour cette modification d’interface. Aucun commit, push ou déploiement.


## Largeur de référence et bandeau Nouveautés, 10/10/2026

La comparaison a utilisé le site Search Console du checkout principal en lecture seule, HEAD `28e8e4ad47fc5fa9db3bcb1cbd3471958abc5b2b`, avec une prévisualisation locale temporaire de sa compilation. Cette prévisualisation a été arrêtée après les mesures. Le serveur wt-sweep reste accessible à http://localhost:49331/fr/.

Le menu, le bandeau et le hero wt-sweep reprennent maintenant l’enveloppe de 90rem et le padding de la référence. Les bords du contenu mesurés dans les deux sites sont identiques : 16–374px à 390px, 72–1368px à 1440px et 352–1696px à 2048px. Les sections ordinaires conservent 75rem, comme la référence ; les limites de largeur de prose et la composition du hero restent propres à wt-sweep.

Le nouveau bandeau FR/EN renvoie aux nouveautés. Sa date vient du marqueur canonique `unreleased-updated` dans le changelog relu, jamais de l’heure de compilation. Le statut reste « Non publié » / « Unreleased ». Les dates manquantes ou impossibles font échouer la compilation. Aucune release n’est annoncée.

Preuves : échec initial du contrôle de largeur avec un écart de 48px à 1440px, puis 34 tests navigateur réussis ; les 6 nouveaux cas ont été rejoués après avoir lié leur date attendue à la source. Les 7 tests du site passent, incluant source documentaire, date du bandeau, conservation d’une démo sur un repo Git de test, liens construits et origine HTTPS réservée. Astro : 41 fichiers, aucune erreur ni avertissement ; compilation finale sans avertissement, 31 pages. Le cache documentaire a été reconstruit pour retirer les anciens IDs après régénération.

Six captures FR contrôlées à 390/1440/2048px dans les deux thèmes, plus les deux menus mobiles et la référence Search Console à 1440px. Aucun débordement horizontal, alignements vérifiés, mouvement réduit à 0s. Axe, clavier, changement de langue, recherche documentaire et navigation sans JavaScript passent dans la suite navigateur. Les captures et mesures restent dans `/private/tmp/wt-updates-visuals`.

Fichiers modifiés pour cette adaptation : `src/components/LandingPage.astro`, nouveau `src/components/UpdatesBanner.astro`, `src/styles/global.css`, nouveau `src/data/updates.mjs`, `tests/browser/site.spec.ts`, nouveau `tests/updates.test.mjs`, `package.json`, `content/en/changelog.md`, `content/fr/changelog.md`, `published-pages.json`, `README.md`, ce registre et `CHANGELOG.md` du repo. Les vérifications Rust/Python produit déjà consignées restent historiques ; elles ne sont pas rejouées pour cette adaptation d’interface. Aucun commit, push ni déploiement.


## Préparation du premier commit et push, 10/10/2026

Livraison autorisée par l’utilisateur sur la branche dédiée `codex/wt-sweep-site`, remote `origin` (`git@github.com:FlorianBruniaux/wt-sweep.git`). Le SHA de `main` distant relu avant livraison est identique à la base produit `c3e38f7810424ae57f58d6ea10c4db996e9122fb`. La sélection porte sur 90 fichiers : site, deux documents de conception/plan, index documentaire et changelog ; les changements du checkout principal restent hors de cette livraison.

Validation complète rejouée avant commit : 7 tests documentaires/démo/HTML/origine HTTPS, 34 tests navigateur, contrôle Astro de 41 fichiers sans erreur ni avertissement, compilation de 31 pages et contrôle du diff préparé réussis. Gitleaks ne détecte aucun secret dans le diff préparé. L’audit antérieur des captures et artefacts synthétiques reste distinct de ce contrôle du diff.

Le lancement de `pnpm verify` a signalé `ERR_PNPM_IGNORED_BUILDS` pour les scripts d’installation esbuild/sharp avant d’exécuter la suite ; la commande finale et tous les contrôles du site ont terminé avec succès sur les dépendances déjà installées. Aucun script supplémentaire n’a été approuvé et aucun réglage global modifié. Une installation entièrement neuve avec cette politique d’approbation reste non validée. Aucun déploiement ni validation de CI distante ne sont déduits des contrôles locaux.


## Domaine public et préparation GitHub Pages, 10/10/2026

Le repo a été rendu public par l’utilisateur. DNS relu : `wt-sweep.bruniaux.com` pointe vers `florianbruniaux.github.io`. GitHub Pages est configuré avec source `workflow` et ce domaine ; le certificat est dans l’état `new` à cette observation. Aucun site n’a encore été déployé et HTTPS n’est pas encore imposé pour le domaine. La licence FSL-1.1-ALv2 est conservée.

Le nouveau workflow `Publish website` accepte uniquement une demande manuelle du propriétaire sur `main`. Il utilise un runner standard Ubuntu, Node 22, pnpm 9.15.9 et le lockfile ; il vérifie les sources relues, les diagnostics Astro et les métadonnées de publication avant envoi de `site/dist` à Pages. La politique distante Actions 7082 est active, avec le compte FlorianBruniaux et l’événement `workflow_dispatch` uniquement. Les automatismes intégrés GitHub restent exemptés de cette politique.

Validation observée : actionlint et contrôle du diff réussis ; 7 tests documentaires/démo/HTML/origine HTTPS, 34 tests navigateur et contrôle Astro de 41 fichiers réussis, sans erreur ni avertissement. Une première tentative a utilisé le Python système trop ancien ; la suite complète a ensuite réussi avec `WT_SITE_PYTHON=/opt/homebrew/bin/python3`. Une compilation séparée de 31 pages utilise le domaine réel : adresses canoniques EN/FR, robots et sitemap contrôlés avec le même validateur que le workflow. L’installation sur un runner Ubuntu neuf et le déploiement distant restent non vérifiés. Les contrôles Rust/Python produit historiques ne sont pas déduits de cette validation du site.


## Révision éditoriale FR/EN, 10/10/2026

La relecture suit `~/.claude/ANTI_AI.md` : phrases complètes, termes usuels des développeurs et conservation du sens technique. Le site et ses guides français emploient repo, worktree, commit et skill. Les fixtures sont décrites comme des repos de test temporaires ; les processus qui écrivent dans un repo ne sont plus appelés « écrivains ». Les textes de présentation anglais et français, descriptions des pages, prompts et titres du menu documentaire ont été revus. La licence et le changelog canonique restent en anglais.

Contrôles observés : 7 tests documentaires, de démo et de HTML réussis ; 34 tests navigateur réussis en français et anglais, avec thèmes clair/sombre, navigation au clavier, Axe, recherche et affichage mobile. Astro contrôle 41 fichiers sans erreur ni avertissement et construit 31 pages. Les captures françaises à 390 et 1440 px, ainsi que le menu documentaire, ont été relues. Les liens internes et leurs ancres résolvent dans le site construit.

Une comparaison avec le commit précédent confirme que les blocs de code littéraux, chiffres et identifiants complets des observations dans la documentation n’ont pas changé. Seuls les prompts en texte libre sont révisés. Formatage Rust, Clippy et suite Cargo passent ; les 22 fichiers du harnais restent synchronisés. Ces contrôles locaux ne constituent pas une validation du déploiement distant.


## Guides du rapport et intégration de main, 10/10/2026

Les guides anglais et français reprennent la destination HTML sur le Bureau,
`--open`, le récapitulatif disque et les liens GitHub/GitLab. Le manifeste des
sources relues est actualisé ; les captures et la démo publiées gardent leurs
provenances historiques. Aucun déploiement n’est lancé dans ce lot.

`pnpm verify` réussit : 7 tests de documentation, démo, HTML et origine HTTPS,
34 tests Chromium, contrôle Astro de 41 fichiers sans erreur ni avertissement,
et compilation de 31 pages. Les dépendances verrouillées ont été installées
depuis le cache avec les scripts d’installation désactivés.

La première passe du navigateur échouait sur six attentes du bandeau : le
commit `765a5d9` avait remplacé « Development version » par la version Cargo,
sans adapter le test. L’assertion vérifie maintenant cette version tout en
conservant les contrôles de disposition, de date et de navigation. La seconde
passe complète réussit. Ces contrôles locaux ne prouvent pas un déploiement.


## Documentation du dashboard et CI, 11/10/2026

Les pages rapport, disque et contrat EN/FR décrivent le dashboard par volume,
la liste d’inventaire et la recherche clavier. Les deux pages de changelog
reprennent la source canonique. Le manifeste relu est mis à jour ; ces ajouts
sont explicitement postérieurs à l’archive alpha publiée.

Validation locale : 8 tests documentaires, de compilation et de métadonnées
publiques, 34 tests Chromium, Astro sans erreur ni avertissement et 31 pages
construites. Les essais utilisent Python 3.14 et les dépendances verrouillées.
Le Python système 3.9 et l’écriture de cache dans un checkout de référence ont
été refusés lors des premières tentatives ; la validation a repris dans une
copie de dépendances appartenant au clone isolé. Ces contrôles ne prouvent pas
une publication. La [politique CI actuelle](../docs/ci.md) remplace la restriction
aux seuls événements manuels décrite dans l’entrée historique ci-dessus.
