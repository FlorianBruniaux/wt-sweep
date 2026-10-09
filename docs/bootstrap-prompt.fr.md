# Installer Worktree Sweep avec un assistant

[Index documentaire](README.md) · [Guide du skill](assistant-skill.md) · [Tutoriel CLI](tutorial.fr.md)

Collez le prompt ci-dessous dans Claude Code, Codex ou un autre assistant ayant
accès aux fichiers et au terminal de votre Mac. Un chat sans ces outils peut
expliquer les étapes, mais ne peut pas installer ou exécuter la CLI.

Le clonage récupère le code et les deux versions du skill. Aucune copie dans les
skills globaux n’est nécessaire. L’invocation native `/wt-sweep` ou `$wt-sweep`
dépend du client et de sa confiance dans le projet ; une lecture explicite du
fichier permet de poursuivre sans prétendre avoir activé cette intégration.

Si le dépôt est encore privé lors de l’installation, l’assistant doit utiliser
un accès GitHub déjà configuré, ou vous guider pour l’établir, sans demander de
secret dans le chat.
Les téléchargements concernent l’outil et ses dépendances. L’analyse de vos
projets reste en lecture seule et ne rafraîchit pas leurs références distantes.

## Prompt à copier

```text
Installe Worktree Sweep et accompagne-moi pour examiner mes worktrees Git :
où ils sont, l’espace qu’ils occupent et le travail à préserver avant nettoyage.
Dépôt officiel : https://github.com/FlorianBruniaux/wt-sweep

Je t’autorise à cloner cet outil et à télécharger ses dépendances Rust.
Ne supprime aucun worktree, fichier ou branche. Ne modifie pas ma configuration
globale, mes hooks ou mes dépôts à analyser.

1. Vérifie macOS, /usr/bin/git, Cargo et Rustup. Si un prérequis manque,
   explique le nécessaire et demande mon accord avant son installation.
   Réutilise le checkout de wt-sweep déjà indiqué dans notre conversation ;
   sinon clone le dépôt dans ~/.local/share/wt-sweep si ce chemin est libre.
   Si le dossier existe, vérifie son origine et son état avant de le réutiliser.
   Préserve tout changement local et n’écrase jamais un dossier existant.
   Si l’accès GitHub manque, guide-moi sans demander de jeton dans le chat.

2. Lis les instructions du dépôt et README.fr.md. Charge explicitement le
   skill .agents/skills/wt-sweep/SKILL.md pour Codex, ou
   .claude/skills/wt-sweep/SKILL.md pour Claude Code. Pour un autre client,
   lis harness/skills/wt-sweep/SKILL.md. Lis aussi ses références commands.md,
   reports.md et model-policy.md dans le sous-dossier references/.
   Les fichiers sont déjà téléchargés avec le clone. N’installe rien globalement
   et ne prétends pas que le client a reconnu le skill sans l’avoir vérifié.

3. Depuis le checkout, lance cargo fetch --locked si les dépendances manquent,
   puis cargo build --release --locked --offline. Rustup peut télécharger la
   version fixée dans rust-toolchain.toml ; ce téléchargement est autorisé.
   Conserve le chemin absolu du binaire, le commit source et l’état du checkout.
   Vérifie --version, --help et review --help. Ne contourne pas un échec.

4. Exécute la démo jetable de docs/getting-started.md pour vérifier le parcours.
   Sur une collecte complète, elle doit donner 2 keep, 1 cleanup_candidate et
   0 verify. Si le résultat diffère, explique pourquoi avant d’enchaîner.
   Puis réutilise les chemins et cibles déjà confirmés dans notre conversation.
   Demande seulement ceux qui manquent. Ne devine aucune branche d’intégration
   et ne lance pas de scan de toute la machine sans demande explicite.

5. Lance review sur le périmètre confirmé avec --budget standard, --summary,
   --disk-usage et --html. Crée avec mktemp un dossier de rapports unique hors
   des dépôts inspectés et passe-le à --run-dir. Utilise les recettes du skill,
   vérifie le code de sortie puis les artefacts et leur reçu. Ouvre report.html.
   Pour plusieurs projets, conserve les cibles propres à chaque famille Git.

6. Donne le bilan par projet, puis lis cinq worktrees à la fois : chemin,
   branche, taille et complétude, modifications locales, verdict CLI, motif et
   prochaine vérification. Explique keep, verify et cleanup_candidate en français.
   Conserve les inconnues ; la taille allouée ne prédit pas le gain sur APFS.
   Pour une limite de collecte, lis le rapport avant de proposer une reprise
   ciblée : --worktree et --budget thorough pour les preuves, --disk-root et
   --disk-budget pour le disque. Une simple vue filtrée ne réduit pas la collecte.

Applique la politique modèle/raisonnement du skill avec les capacités réellement
disponibles. Un agent suffit au départ ; au plus deux sous-agents peuvent lire
des lots distincts d’un rapport sauvegardé, sans lancer de scans concurrents.
Annonce les étapes et les blocages utiles, sans recopier tous les logs.
Termine par les chemins des rapports et les vérifications prioritaires.
Propose les actions de conservation, sans exécuter commit, stash ou reset dans
les projets examinés. Aucun candidat n’autorise une suppression : apply reste
indisponible. L’installation ne t’autorise pas à fetch/prune/gc ces projets.
```

## Ce que ce parcours établit

| Étape | Preuve attendue |
| --- | --- |
| Acquisition | Checkout identifié, commit source et changements locaux signalés |
| Skill | Chemin du fichier effectivement lu ; reconnaissance native vérifiée séparément |
| Compilation | Code de sortie réussi et chemin du binaire compilé |
| Démo | Résultat observé sur le dépôt jetable, avec ses éventuelles limites |
| Analyse réelle | Périmètre et cibles confirmés, rapports et reçu vérifiés |
| Lecture | Verdicts conservés, lignes lues et restantes, prochaines vérifications |

Ce document est un prompt de démarrage. Il ne crée pas d’installateur et ne
change pas le skill existant. Python 3.11+ sert aux tests de développement et à
certains exemples optionnels ; il n’est pas requis pour la CLI ou la démo shell.
La présence des fichiers du skill ne prouve pas leur découverte native.
