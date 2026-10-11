wt-sweep recueille des observations sur les fichiers et les objets Git, puis applique ses règles de décision à ces faits. Le rapport conserve les preuves manquantes, incomplètes ou contradictoires.

## Conditions de décision

`keep` prime lorsqu’une preuve de conservation existe. `cleanup_candidate` exige toutes les conditions C1–C5 actuelles. Sinon la ligne reste `verify`.

Lisez ensemble `keep_reasons`, `missing_conditions`, les contrôles de l’état actuel, le résumé local, l’historique Git, les analyses approfondies et l’historique privé. Le checkout principal, le travail local et une PR ou MR ouverte dont l’association a été vérifiée sont des motifs de conservation. L’âge du worktree, un patch-id identique, une PR ou MR fermée et une suggestion de modèle ne prouvent jamais l’intégration.

Le [contrat canonique du triage](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/contracts.md#triage-policy) définit C1–C5. Le site reprend ce contrat sans changer ses règles.

## La couverture n’est pas un instantané

Une découverte terminée ne prouve pas que toute la machine a été inventoriée ni que toutes les données ont été lues au même instant. Les dossiers cachés et ignorés sont inclus ; les refus d’accès, les limites atteintes et les preuves non recueillies restent signalés.

`0` n’est exact que pour une comparaison locale complète. `>=N` est une borne inférieure. Un compte à zéro issu d’une comparaison incomplète reste `UNKNOWN`.

## Reçus et conservation

`run.json` lie octets, tailles et SHA-256 des artefacts sauvegardés. Il ne prouve ni sauvegarde restaurable, ni restauration indépendante, ni dernière synchronisation disque réussie.

Les contrôles nécessaires pour bloquer les écritures concurrentes, vérifier la durabilité des sauvegardes et les restaurer indépendamment restent à valider. `apply` est indisponible et aucun verdict n’autorise une suppression.

## Limites de plateforme

La CLI active un contrôle macOS contre la matérialisation des fichiers dataless. Les vrais placeholders FileProvider et refus TCC restent non vérifiés. Un appel bloqué dans le noyau peut dépasser le délai fixé par la CLI. Les [tailles disque](/fr/docs/disk/) ne promettent jamais un espace APFS récupérable.

## Contexte de forge

Les URL de dépôt, branche et commit viennent de métadonnées locales capturées. Elles ne sont pas vérifiées auprès du serveur. Les PR/MR affichées exigent une association exacte avec des observations importées ; les liens de remotes ne changent pas le triage. [Liens du rapport et provenance](/fr/docs/report/#liens-github-et-gitlab).

## Compatibilité du format disque

Depuis `v0.1.0-alpha.2`, les sources peuvent enregistrer le bloc facultatif `disk_usage.storage`. Le nouveau lecteur accepte les anciens inventaires sans changer leur empreinte canonique. Les anciens lecteurs stricts peuvent refuser les fichiers enrichis. Des données de volume inconnues ou incohérentes empêchent le calcul des pourcentages, sans effacer tailles ni lignes. La capacité ne change pas C1–C5.
