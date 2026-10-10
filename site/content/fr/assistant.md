L’assistant explique les preuves CLI. Il ne remplace pas les verdicts déterministes et n’autorise aucun retrait. La CLI fonctionne sans modèle ; la lecture assistée utilise votre session client habituelle.

## Ouvrir le bon checkout

Ouvrez une session dans le checkout wt-sweep contenant le skill de dépôt. Le dépôt analysé peut être ailleurs. Une session ouverte seulement dans cet autre dépôt ne dispose pas nécessairement du skill.

| Client | Invocation | Fichier du dépôt |
| --- | --- | --- |
| Claude Code | `/wt-sweep` | `.claude/skills/wt-sweep/SKILL.md` |
| Codex | `$wt-sweep` | `.agents/skills/wt-sweep/SKILL.md` |

La découverte native dépend de la version du client et de la confiance accordée au projet. Après ajout, ouvrez une nouvelle session et vérifiez le sélecteur de skills. Une validation de fichiers ne prouve pas l’activation native.

## Commencer par la démo

```text
$wt-sweep Lis docs/getting-started.md et vérifie la CLI compilée.
Exécute uniquement sa démo jetable, puis explique le rapport HTML sauvegardé.
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

Le chemin doit désigner le dossier `run-*` contenant reçu et artefacts. Un rapport historique ne suffit pas pour décider d’une suppression actuelle. Les [revues ciblées](/fr/docs/targets/) peuvent recueillir une preuve manquante dans une nouvelle exécution.
