Un checkpoint poursuit la découverte de répertoires sur racines explicites après une limite ou une interruption. Il ne reprend ni mesure disque, ni hachage local, ni preuves de revue.

## Sauvegarder et poursuivre

Remplacez la racine d’exemple par votre chemin absolu choisi. Gardez la sortie hors de cette racine. Lancez la seconde commande uniquement si le premier passage a publié un checkpoint et si la découverte reste incomplète.

```sh
scan_dir="$(mktemp -d /private/tmp/wt-sweep-discovery.XXXXXX)"
./target/release/wt-sweep scan --root /absolute/path/to/projects \
  --checkpoint-out "$scan_dir/pass-1.json" --run-dir "$scan_dir" --html

./target/release/wt-sweep scan --resume "$scan_dir/pass-1.json" \
  --checkpoint-out "$scan_dir/pass-2.json" --run-dir "$scan_dir" --html
```

Le checkpoint fournit les racines d’origine. Des racines répétées facultatives doivent correspondre exactement, ordre compris. Les nouveaux noms de sortie évitent d’écraser un passage précédent.

Seul `scan` sur racines explicites accepte les checkpoints. `--repo`, `--all` et `review` ne les acceptent pas. Un très large répertoire peut demander un plafond d’entrées plus élevé dans la plage autorisée, ou une nouvelle racine plus étroite.

## Lire le passage suivant

Les preuves des familles connues sont relues. Les répertoires terminés gardent des observations historiques ; un dépôt créé ensuite à l’intérieur peut ne pas être retrouvé par la reprise. Identités des répertoires et digest sont vérifiés. Une identité modifiée peut provoquer un refus.

Lisez couverture, chemins en attente et omissions de publication. Chaque passage garde ses intervalles et leur ensemble ne forme jamais un instantané atomique. Ni checkpoint ni terminaison n’autorise un nettoyage.
