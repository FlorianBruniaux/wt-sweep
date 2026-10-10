La collecte observe fichiers et objets Git. Le jugement déterministe lit ces faits séparément. Les preuves manquantes, incomplètes et contradictoires restent visibles.

## Conditions de décision

`keep` prime lorsqu’une preuve de conservation existe. `cleanup_candidate` exige toutes les conditions C1–C5 actuelles. Sinon la ligne reste `verify`.

Lisez ensemble `keep_reasons`, `missing_conditions`, contrôles actuels, résumé local, histoire, observations profondes et historique privé. Checkout principal, travail local et demande ouverte précisément associée ont des motifs explicites de conservation. Âge, patch-id, demande close ou suggestion de modèle ne prouvent jamais l’intégration.

Le [contrat canonique du triage](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/contracts.md#triage-policy) définit C1–C5. Le portail ne crée pas une autre règle.

## La couverture n’est pas un instantané

Un parcours de découverte terminé n’est ni un instantané de toute la machine, ni une photographie atomique. Répertoires cachés et ignorés sont inclus ; refus d’accès, limites et preuves non collectées restent signalés.

`0` n’est exact que pour une comparaison locale complète. `>=N` est une borne inférieure. Un zéro incomplet reste `UNKNOWN`.

## Reçus et conservation

`run.json` lie octets, tailles et SHA-256 des artefacts sauvegardés. Il ne prouve ni sauvegarde restaurable, ni restauration indépendante, ni dernière synchronisation disque réussie.

Les conditions d’exclusion des écrivains, sauvegarde durable et restauration indépendante n’ont pas passé leurs gates. `apply` reste indisponible. Aucun verdict n’autorise un retrait.

## Limites de plateforme

La CLI établit une garde macOS native contre la matérialisation dataless. Les vrais placeholders FileProvider et refus TCC restent non vérifiés. Un appel noyau bloqué peut dépasser un délai utilisateur. Les [tailles disque](/fr/docs/disk/) ne promettent jamais un espace APFS récupérable.
