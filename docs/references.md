# Références et décisions de réutilisation

La recherche du 04/10/2026 porte sur la documentation et une lecture ciblée du
code. Aucun de ces produits n'a été exécuté pour valider notre contrat complet.
L'absence de candidat équivalent vérifié n'est pas une preuve d'absence du marché.

| Projet | Fonction examinée | Limite pour notre contrat |
|---|---|---|
| [agentrinse](https://github.com/vincentkoc/agentrinse) | Plan, revalidation, quarantaine et undo | Racines explicites ; détachés et ignorés refusés |
| [stalewood](https://github.com/retif/stalewood) | Découverte croisée et worktrees abandonnés | Exclusions de parcours et intégration dans une autre branche |
| [ward](https://github.com/michaelmillar/ward) | Bundle et essai de restauration | Fichiers ignorés exclus de l'archive complémentaire |
| [gh-reaper](https://github.com/ai-ecoverse/gh-reaper) | Inventaire et nettoyage de worktrees | Couverture bornée et protocole de conservation différent |
| [workroot](https://github.com/fridiculous/workroot) | Inventaire transversal | Conservation complète non établie ici |
| [Worktrunk](https://github.com/max-sixty/worktrunk) | Gestion de worktrees et sous-commandes | Intégration possible, pas une preuve de conservation |
| [git-wipe](https://github.com/noirbizarre/git-wipe) | Nettoyage de branches et worktrees | Portée dépôt et opérations hors de notre MVP |

Points de code vérifiés : [plan agentrinse](https://github.com/vincentkoc/agentrinse/blob/6d5e047e00ef0193f6b75b952060a80ea310b1a4/src/core/plan-verification.ts),
[éligibilité agentrinse](https://github.com/vincentkoc/agentrinse/blob/6d5e047e00ef0193f6b75b952060a80ea310b1a4/src/adapters/git/adapter.ts),
[archive ward](https://github.com/michaelmillar/ward/blob/246d82d7a3c37cc5f76c1687fd3487f4352c1914/src/archive.rs).

Le harness s'inspire des pratiques examinées dans Agent Router (sources communes,
rôles bornés et parité), cc-copilot-bridge (projection Claude/Codex et tests
temporaires), RTK (preuves différentielles Git et propagation des erreurs) et
ccboard (fraîcheur du contexte et observation du CLI). Cette mention décrit des
choix de conception ; aucun module produit ni configuration personnelle n'a été
copié depuis ces projets.

L'entrée optionnelle `wt sweep` pourra utiliser les
[sous-commandes Worktrunk](https://worktrunk.dev/extending/#custom-subcommands).
Elle ne bloque pas le CLI autonome. Disponibilité du nom sur les registres et
choix de licence restent à vérifier avant publication.
