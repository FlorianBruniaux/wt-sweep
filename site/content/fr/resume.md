Un checkpoint permet de reprendre la recherche de repos dans les dossiers choisis après une limite atteinte ou une interruption. Il ne reprend ni les mesures disque, ni le hachage local, ni les preuves de l’analyse.

## Sauvegarder et poursuivre

Remplacez la racine d’exemple par votre chemin absolu choisi. Gardez la sortie hors de cette racine. Lancez la seconde commande uniquement si le premier passage a publié un checkpoint et si la découverte reste incomplète.

```sh
scan_dir="$(mktemp -d /private/tmp/wt-sweep-discovery.XXXXXX)"
./target/release/wt-sweep scan --root /absolute/path/to/projects \
  --checkpoint-out "$scan_dir/pass-1.json" --run-dir "$scan_dir" --html

./target/release/wt-sweep scan --resume "$scan_dir/pass-1.json" \
  --checkpoint-out "$scan_dir/pass-2.json" --run-dir "$scan_dir" --html
```

Le checkpoint fournit les racines d’origine. Si vous indiquez de nouveau les racines, elles doivent correspondre exactement, dans le même ordre. Les nouveaux noms de sortie évitent d’écraser un passage précédent.

Seul `scan` sur racines explicites accepte les checkpoints. `--repo`, `--all` et `review` ne les acceptent pas. Un dossier qui contient beaucoup d’entrées peut nécessiter une limite plus élevée, dans la plage autorisée, ou un nouveau parcours sur un dossier plus restreint.

## Lire le passage suivant

wt-sweep relit les preuves des familles Git déjà connues. Les répertoires terminés gardent des observations historiques ; un repo créé ensuite à l’intérieur peut ne pas être retrouvé par la reprise. wt-sweep vérifie l’identité des dossiers et l’empreinte du checkpoint. Une identité modifiée peut provoquer un refus.

Lisez la couverture, les chemins en attente et les données absentes des rapports publiés. Chaque passage conserve ses intervalles d’observation ; les regrouper ne fournit pas un instantané atomique. Ni le checkpoint ni la fin du parcours n’autorisent un nettoyage.
