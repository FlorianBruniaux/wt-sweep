Repérez les worktrees, mesurez leur taille et consultez les preuves qui justifient chaque verdict. La CLI macOS actuelle reste en lecture seule et fonctionne sans modèle d’IA, service permanent ni compte GitHub ou GitLab.

## Premier essai

1. [Compiler les sources](/fr/docs/installation/).
2. [Essayer sur un repo de test](/fr/docs/demo/).
3. [Lire le rapport HTML hors ligne](/fr/docs/report/).
4. [Choisir les dossiers et les cibles d’intégration](/fr/docs/targets/) pour votre propre analyse.

## Lire un résultat

- **keep** : un travail observé ou un motif de conservation prime.
- **verify** : une preuve nécessaire à la décision manque.
- **cleanup_candidate** : les conditions C1–C5 actuelles sont établies. Il reste à vérifier si le worktree est encore utilisé.

Aucun verdict n’autorise la suppression d’un worktree. `apply`, sauvegarde et restauration restent indisponibles. Un commit identique ne préserve pas les notes non suivies.

Les [mesures disque](/fr/docs/disk/) indiquent leur périmètre et les données qui n’ont pas pu être recueillies. Les [limites des preuves](/fr/docs/evidence/) expliquent pourquoi une commande réussie peut encore contenir des inconnues.

[Claude Code et Codex](/fr/docs/assistant/) peuvent vous aider à lire les résultats de la CLI par petits groupes.
