Claude Code ou Codex peut expliquer les preuves de la CLI, sans remplacer ses verdicts déterministes ni autoriser une suppression. La CLI fonctionne sans modèle d’IA ; la lecture assistée utilise votre session habituelle.

## Ouvrir le bon checkout

Ouvrez une session dans le checkout wt-sweep qui contient le skill. Vous pouvez analyser un repo situé ailleurs, mais une session ouverte uniquement dans cet autre repo ne dispose pas nécessairement du skill.

| Client | Invocation | Fichier du repo |
| --- | --- | --- |
| Claude Code | `/wt-sweep` | `.claude/skills/wt-sweep/SKILL.md` |
| Codex | `$wt-sweep` | `.agents/skills/wt-sweep/SKILL.md` |

La reconnaissance du skill dépend de la version du client et de la confiance accordée au projet. Après son ajout, ouvrez une nouvelle session et vérifiez le sélecteur de skills. Le fait que les fichiers soient valides ne prouve pas que le client charge le skill.

## Commencer par la démo

```text
$wt-sweep Lis docs/getting-started.md et vérifie la CLI compilée.
Exécute uniquement sa démo sur un repo de test, puis explique le rapport HTML sauvegardé.
Conserve les verdicts originaux et les lacunes de couverture.
Ne parcours pas ma machine et ne supprime rien.
```

Remplacez `$wt-sweep` par `/wt-sweep` dans Claude Code.

## Lire un rapport existant

```text
$wt-sweep Lis le dossier de rapport /chemin/absolu/run-existant.
Ne relance pas de scan. Vérifie les artefacts contre run.json.
Donne périmètre et couverture, puis lis cinq worktrees à la fois.
Pour chaque verify, nomme la condition manquante et le prochain contrôle utile.
Distingue les observations enregistrées de l’état actuel, qui n’a pas été revérifié.
```

Le chemin doit désigner le dossier `run-*` contenant le reçu de publication et les fichiers du rapport. Un rapport historique ne suffit pas pour décider d’une suppression actuelle. Les [analyses ciblées](/fr/docs/targets/) peuvent recueillir une preuve manquante lors d’une nouvelle exécution.

Les rapports peuvent inclure des liens GitHub/GitLab et les états capturés des PR/MR importées et associées. Ces éléments ne prouvent ni publication, ni intégration, ni éligibilité au nettoyage. [Lire la provenance des liens](/fr/docs/report/#liens-github-et-gitlab).
