# Disk Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking. The user selects the execution method before implementation.

**Goal:** Ajouter les jauges de capacité et d'allocation mesurée au rapport HTML, avec la couverture et la liste directement visible.

**Architecture:** Capturer les statistiques sur les descripteurs déjà ouverts par le collecteur disque. Compter les octets uniques par device, puis rendre ces données hors ligne avec une arithmétique entière. Chaque volume reste distinct.

**Tech Stack:** Rust 1.95, édition 2024, stdlib et dépendances existantes verrouillées ; HTML/CSS natifs, aucune nouvelle dépendance.

**Spec:** [Tableau de bord disque](../specs/2026-10-11-disk-dashboard-design.md), approuvé le 11 octobre 2026.

Statut : implémentation locale achevée, revue indépendante effectuée et constats
corrigés. Les contrôles ont été exécutés ; leurs limites figurent dans le bilan
ci-dessous. Base inspectée :
`ec40d8f9ef1daf0b2c893551e1b37da91fd11d16`, avec les modifications HTML locales
préservées. Les interfaces ci-dessous décrivent le plan approuvé ; la réalisation
transmet aussi le contrôle de budget à `observe_volume`, avant et après chaque
opération native.

## Global Constraints

- « Les informations sont capturées uniquement avec `--disk-usage`, pendant la collecte existante. »
- « `plan` et le rendu HTML n'interrogent jamais la machine. »
- « Une observation est tentée par device, au plus 4 096 devices, dans le budget restant de la collecte. »
- « Aucune capacité ni quantité disponible n'est additionnée entre volumes. »
- « Les filtres de la liste ne changent pas ces chiffres source. »
- « Aucun scan global n'est lancé pendant le développement. »
- Préserver C1-C5, les limites, les octets des chemins et les OID complets.
- Préserver `filesystem boundary not traversed`. Un autre volume se mesure depuis ses propres racines enregistrées.
- Aucun fetch, prune, gc, nettoyage réel, configuration globale ou changement de Cargo.
- Relire `README.md`, `docs/roadmap.md`, `docs/plan.md`, `docs/implementation.md`, `docs/contracts.md` et `docs/targets-and-filters.md` avant le code.

## Review Focus

1. Ancien inventaire sans capacité : digest intact et pourcentage explicitement indisponible (tâches 1 et 4).
2. Deux volumes APFS annoncent la même capacité : deux cartes, aucune capacité cumulée (tâche 4).
3. Identité du fd contradictoire, erreur native ou budget atteint : octets conservés, capacité inconnue (tâche 3).
4. Quota dépassé ou entier au-delà de la précision JavaScript : valeur exacte conservée, seule la barre est bornée (tâches 1 et 4).
5. Hardlinks, racines imbriquées et sélection partielle : conservation du total, omissions explicites et jauges source inchangées (tâches 2 et 5).

## Fichiers et responsabilités

| Fichier | Responsabilité |
| --- | --- |
| `src/disk/storage.rs` (nouveau) | Contrat, sommes et ratios purs. |
| `src/disk/storage/native.rs` (nouveau) | Sonde par fd, backend injectable et frontière native privée. |
| `src/disk.rs` | Champ optionnel, comptage par device, tentative bornée par device. |
| `src/presentation/html/dashboard.rs` (nouveau) | Composant HTML utilisant le `Html` borné existant. |
| `src/presentation/html.rs`, `src/presentation/html/report.css` | Placement et styles, liste ouverte conservée. |
| `tests/disk_usage.rs`, `tests/html_report.rs`, `tests/test_html_cli.py` | Conservation, rendu, rejeu et publication. |
| `tests/html-dashboard.cjs` (nouveau) | Vérification navigateur sur rapport synthétique. |
| `tests/review.rs`, `tests/triage.rs`, `tests/triage_presentation.rs` | Compatibilité des constructions de `DiskUsage`. |
| `docs/contracts.md`, `docs/disk-usage.md`, `docs/native-boundaries.md`, `docs/html-report-plan.md`, `CHANGELOG.md` | Contrat et comportements réellement livrés. |

Les tâches sont séquentielles et partagent un intégrateur. Aucun changement
des versions des enveloppes inventaire/revue ou du harness.

## Base à conserver

- [x] Capturer HEAD, statut, diff binaire et hashes des fichiers touchés, y compris non suivis, dans un répertoire temporaire appartenant à l'exécution. Ne jamais réinitialiser le checkout.
- [x] Relever `rg -n 'DiskUsage \{' src tests` et comparer les fichiers avant édition. Réconcilier toute écriture concurrente, préserver les changements HTML antérieurs et utiliser des pathspecs explicites. Aucun commit ou push sans autorisation correspondante.

### Tâche 1 : contrat optionnel et calculs

**Files:** créer `src/disk/storage.rs` ; modifier `src/disk.rs`, `tests/disk_usage.rs`, `tests/html_report.rs`, `tests/review.rs`, `tests/triage.rs`, `tests/triage_presentation.rs` et `docs/contracts.md`.

**Interfaces:** ajouter `pub mod storage;` et `DiskUsage.storage: Option<storage::StorageUsage>` avec `#[serde(default, skip_serializing_if = "Option::is_none")]`. Types proposés, avec `Clone, Debug, Serialize, Deserialize` et `serde(deny_unknown_fields)` ; importer les types existants `BytePath`, `Identity` et `DiskBytes`.

```rust
pub struct StorageUsage {
    pub schema: String, // wt-sweep.storage/1
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub temporal_consistency: String, // not_guaranteed
    pub devices: Vec<DeviceStorage>, // ordre croissant par device
}
pub struct DeviceStorage {
    pub device: u64,
    pub bytes: DiskBytes,
    pub volume: Option<VolumeSpace>,
    pub unavailable_reason: Option<String>,
}
pub struct VolumeSpace {
    pub anchor: Identity,
    pub anchor_path: BytePath,
    pub mount_path: Option<BytePath>,
    pub source: Option<BytePath>,
    pub filesystem: Option<BytePath>,
    pub identity_verified: bool,
    pub capacity_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub method: String, // fstatfs
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub gaps: Vec<String>, // maximum huit raisons de 2 048 octets chacune
}
pub fn percentage_tenths(part: u64, total: Option<u64>) -> Option<u128> {
    let total = u128::from(total.filter(|n| *n > 0)?);
    Some((u128::from(part) * 1000 + total / 2) / total)
}
pub(super) fn blocks_to_bytes(blocks: u64, block_size: u32) -> Option<u64> {
    if block_size == 0 || blocks == u64::MAX { return None; }
    blocks.checked_mul(u64::from(block_size))
}
```

- [x] Ajouter le test suivant dans un module `#[cfg(test)]` de `storage.rs`, avec `#[allow(clippy::expect_used)]` limité aux fixtures. Étendre `historical_disk_format_keeps_its_canonical_digest` avec `assert!(restored.storage.is_none())` et une assertion d'absence de la clé dans la sérialisation, en conservant l'oracle SHA-256 actuel.

```rust
#[test]
fn percentages_keep_unknowns_and_large_values() {
    assert_eq!(percentage_tenths(100, Some(2000)), Some(50));
    assert_eq!(percentage_tenths(251, Some(100)), Some(2510));
    assert_eq!(percentage_tenths(u64::MAX, Some(u64::MAX)), Some(1000));
    assert_eq!(percentage_tenths(1, Some(16)), Some(63));
    assert_eq!(percentage_tenths(1, Some(0)), None);
    assert_eq!(percentage_tenths(1, None), None);
    assert_eq!(blocks_to_bytes(10, 4096), Some(40960));
    assert_eq!(blocks_to_bytes(u64::MAX - 1, 4096), None);
    assert_eq!(blocks_to_bytes(u64::MAX, 1), None);
    assert_eq!(blocks_to_bytes(10, 0), None);
}
```

- [x] Constater le rouge avec `rtk proxy cargo test --locked --offline --lib disk::storage` : symboles ou champ manquants avant l'ajout du contrat.
- [x] Implémenter le contrat et ces helpers ; ajouter `storage: None` aux constructions historiques, y compris celle du collecteur. Vérifier le vert avec la même commande et `rtk proxy cargo test --locked --offline --test disk_usage historical_disk_format_keeps_its_canonical_digest`.
- [x] Documenter le bloc facultatif, les champs inconnus et la compatibilité : nouveau lecteur vers anciens fichiers, anciens lecteurs stricts pouvant refuser les nouveaux fichiers. Un sous-schéma inconnu ou incohérent interdit le calcul des jauges mais ne retire pas la liste ni les tailles historiques.

**Livrable :** ancien digest intact, ratios exacts arrondis à une décimale, aucune collecte nouvelle.

### Tâche 2 : octets uniques par device

**Files:** modifier `src/disk.rs`, `src/disk/storage.rs` et `tests/disk_usage.rs`.

**Interfaces:** ajouter `device_bytes: BTreeMap<u64, DiskBytes>` au `Collector`, y compris son constructeur d'unité. Helper interne proposé :

```rust
pub(super) fn add_unique(
    global: &mut DiskBytes,
    devices: &mut std::collections::BTreeMap<u64, DiskBytes>,
    device: u64,
    bytes: &DiskBytes,
) -> Result<(), &'static str>;
```

- [x] Écrire ce test dans `storage.rs` et constater le rouge avec `rtk proxy cargo test --locked --offline --lib disk::storage`.

```rust
#[test]
fn device_sums_are_conserved_and_overflow_changes_nothing() {
    let mut total = DiskBytes::default();
    let mut devices = std::collections::BTreeMap::new();
    assert!(add_unique(&mut total, &mut devices, 7,
        &DiskBytes { logical: 5, allocated: 4096 }).is_ok());
    assert!(add_unique(&mut total, &mut devices, 8,
        &DiskBytes { logical: 7, allocated: 8192 }).is_ok());
    assert_eq!((total.logical, total.allocated), (12, 12288));
    assert_eq!(devices.values().map(|b| b.allocated).sum::<u64>(), 12288);
    let before = serde_json::to_value((&total, &devices)).expect("fixture");
    assert!(add_unique(&mut total, &mut devices, 7,
        &DiskBytes { logical: u64::MAX, allocated: 1 }).is_err());
    assert_eq!(serde_json::to_value((&total, &devices)).expect("fixture"), before);
}
```

- [x] Calculer les quatre sommes avec `checked_add` dans des temporaires ; publier global et device seulement si toutes réussissent. Dans `account`, remplacer l'insertion initiale dans `w.seen` par un contrôle `contains`, préparer le nouveau total de ligne puis insérer dans `w.seen` seulement après réussite. Le code suivant s'insère après le contrôle de la somme de ligne :

```rust
if !self.seen.contains(&id) {
    storage::add_unique(&mut self.unique, &mut self.device_bytes, id.0, &bytes)?;
    self.seen.insert(id);
}
// Affecter ensuite le nouveau total de ligne préparé, sans addition faillible.
```

- [x] Étendre `global_totals_deduplicate_hardlinks_and_nested_registered_worktrees` pour comparer les sommes logique/allouée des devices au total global et vérifier une seule contribution du hardlink. Ajouter un test du collecteur avec deux `Stat` synthétiques de devices distincts : refus du device enfant, attribution du second device depuis sa propre racine enregistrée. Aucune fixture ne monte un système de fichiers.
- [x] Finaliser le bloc avec l'ordre du `BTreeMap`, les timestamps de la collecte et `not_guaranteed`. Chaque device porte provisoirement `volume: None` et la raison `capacity_not_observed`. Les limites d'entrée existantes bornent le nombre de devices comptés ; le plafond de 4 096 vise les tentatives natives, pas l'effacement des totaux. Vérifier avec `rtk proxy cargo test --locked --offline --lib disk::` et `rtk proxy cargo test --locked --offline --test disk_usage`.

**Livrable :** conservation des octets par device et déduplication globale démontrées.

### Tâche 3 : capacité depuis le descripteur vérifié

**Files:** créer `src/disk/storage/native.rs` ; modifier `src/disk/storage.rs`, `src/disk.rs`, `tests/disk_usage.rs` et `docs/native-boundaries.md`.

**Interfaces:** déclarer `pub(super) mod native;` dans `storage.rs`. Les éléments que `disk.rs` appelle utilisent `pub(in crate::disk)` afin d'être visibles depuis ce grand-parent. Backend injectable et observation proposés :

```rust
pub(in crate::disk) struct RawVolumeSpace {
    pub block_size: u32,
    pub blocks: u64,
    pub free_blocks: u64,
    pub available_blocks: u64,
    pub mount_path: Option<BytePath>,
    pub source: Option<BytePath>,
    pub filesystem: Option<BytePath>,
}
pub(in crate::disk) trait VolumeBackend {
    fn identity(&self, fd: std::os::fd::BorrowedFd<'_>)
        -> std::io::Result<Identity>;
    fn snapshot(&self, fd: std::os::fd::BorrowedFd<'_>)
        -> std::io::Result<RawVolumeSpace>;
}
pub(in crate::disk) struct KernelVolumeBackend;
pub(in crate::disk) fn observe_volume(
    fd: std::os::fd::BorrowedFd<'_>, expected: &Identity,
    path: &BytePath, backend: &dyn VolumeBackend,
) -> VolumeSpace;
```

- [x] Écrire les tests injectés de mismatch avant appel, changement après appel, erreur système, compteurs invalides et noms non terminés par NUL. Le backend de test retourne une séquence d'identités et un snapshot configurés ; un fd de répertoire temporaire suffit. L'oracle de refus est :

```rust
let observed = observe_volume(fd.as_fd(), &expected, &path, &backend);
assert!(!observed.identity_verified);
assert!(observed.capacity_bytes.is_none());
assert!(observed.available_bytes.is_none());
assert!(!observed.gaps.is_empty());
```

- [x] Constater le rouge avec `rtk proxy cargo test --locked --offline --lib disk::storage::native`.
- [x] Implémenter `VolumeBackend` pour `KernelVolumeBackend`. `identity` utilise `rustix::fs::fstat`. Le backend macOS appelle `libc::fstatfs` dans un sous-module privé `#[allow(unsafe_code)]` : struct initialisée, fd emprunté vivant et code de retour vérifié. Utiliser les bindings existants de `libc`, sans recopier l'ABI. Sur les autres plateformes, retourner une erreur explicite. La frontière suit ce code proposé, à compléter par la conversion bornée des champs définie dans l'étape suivante :

```rust
// À l'intérieur du sous-module ffi macOS, avec AsRawFd importé :
let mut raw: libc::statfs = unsafe { std::mem::zeroed() };
// SAFETY : raw est initialisé et vivant, fd est emprunté pendant tout l'appel.
let status = unsafe { libc::fstatfs(fd.as_raw_fd(), &mut raw) };
if status != 0 { return Err(std::io::Error::last_os_error()); }
// Lire raw uniquement après le succès, puis convertir en RawVolumeSpace.
```
- [x] Observer dans l'ordre horodatage, identité avant, snapshot, identité après, validation et horodatage final. Les identités doivent égaler `expected` ; sinon invalider toutes les tailles. Calculer les produits via `blocks_to_bytes`, rendre la capacité nulle inconnue, invalider disponible/libre quand supérieurs à la capacité. Décoder les noms jusqu'au NUL dans leurs tableaux natifs bornés ; sans NUL, conserver une raison et aucun chemin tronqué. Ne jamais déduire le device du chemin ou de `fsid`.
- [x] Dans `Collector::root`, tenter la sonde après vérification d'identité et refus dataless, sur le fd déjà ouvert, avant son transfert à `directory`. Ajouter `volume_backend: &dyn VolumeBackend` au collecteur : `collect_selected` utilise une instance locale de `KernelVolumeBackend`, les unités utilisent leur backend injecté. Conserver `probed_devices: BTreeSet<u64>` et `volume_spaces: BTreeMap<u64, VolumeSpace>`. Une tentative échouée consomme la place ; aucun retry, ouverture supplémentaire ou commande externe. Contrôler le budget avant et après ; une réponse tardive garde des tailles inconnues avec `capacity_time_budget`. Une erreur de capacité ne retire aucun octet ni preuve Git.
- [x] Tester avec backend et plafond injectés : une tentative par device même après échec, plafond atteint avec octets conservés, budget déjà épuisé sans appel et réponse tardive invalidée. Enregistrer `capacity_probe_limit`, `capacity_time_budget` ou `capacity_not_observed` pour les devices sans observation. Les diagnostics natifs utilisent huit raisons de 2 048 octets au maximum ; ne pas y recopier de données non bornées.
- [x] Ajouter une preuve native positive sur un unique repo temporaire : identité du fd, capacité positive et disponible inférieure ou égale à la capacité. Vérifier qu'un scan sans `--disk-usage` n'ajoute pas le bloc. Exécuter les unités et `rtk proxy cargo test --locked --offline --test disk_usage`.
- [x] Documenter la nouvelle frontière privée, la politique dataless et l'absence de garantie d'interruption immédiate d'un syscall noyau. Ne pas intégrer les compteurs fluctuants à l'égalité de `MountRecord`.

**Livrable :** succès natif et refus injectés, observations vérifiées attachées au bon device.

### Tâche 4 : jauges HTML hors ligne

**Files:** créer `src/presentation/html/dashboard.rs` ; modifier `src/presentation/html.rs`, `src/presentation/html/report.css`, `tests/html_report.rs`, `tests/html-sections.cjs`, `tests/html-theme.cjs` et `docs/html-report-plan.md`.

**Interfaces:** déclarer `mod dashboard;` dans `html.rs`. Le composant utilise `super::Html` et ses méthodes `raw`, `text`, `path`, donc la limite de publication existante. Interface proposée :

```rust
pub(super) fn render(
    h: &mut super::Html,
    usage: Option<&crate::disk::DiskUsage>,
    items: Option<&[crate::model::ReviewItem]>,
) -> std::fmt::Result;
```

- [x] Ajouter un helper synthétique `dashboard_storage() -> StorageUsage` dans `tests/html_report.rs`. Deux devices `7` et `8`, capacité chacun `2_000_000_000_000`, allocation `100_000_000_000` et `50_000_000_000`, disponible `800_000_000_000` et `1_200_000_000_000`. Utiliser `/synthetic/volume-a` et `/synthetic/volume-b`, identités vérifiées, timestamps fixes. Logique : `1234` puis `0`. Ce test fixe les oracles :

```rust
fn dashboard_storage() -> StorageUsage {
    let inputs = [
        (7u64, b"/synthetic/volume-a".as_slice(), 100_000_000_000, 1234, 800_000_000_000),
        (8u64, b"/synthetic/volume-b".as_slice(), 50_000_000_000, 0, 1_200_000_000_000),
    ];
    StorageUsage {
        schema: "wt-sweep.storage/1".into(),
        started_unix_ms: 101, finished_unix_ms: 150,
        temporal_consistency: "not_guaranteed".into(),
        devices: inputs.into_iter().map(|(device, path, allocated, logical, available)| {
            DeviceStorage {
                device, bytes: DiskBytes { logical, allocated }, unavailable_reason: None,
                volume: Some(VolumeSpace {
                    anchor: Identity { device, inode: device + 10 },
                    anchor_path: bp(path), mount_path: Some(bp(path)), source: None,
                    filesystem: Some(bp(b"apfs")), identity_verified: true,
                    capacity_bytes: Some(2_000_000_000_000),
                    free_bytes: Some(available), available_bytes: Some(available),
                    method: "fstatfs".into(), started_unix_ms: 101,
                    finished_unix_ms: 102, gaps: vec![],
                }),
            }
        }).collect(),
    }
}
#[test]
fn dashboard_keeps_volumes_distinct_and_source_intact() {
    let (mut source, _) = fixture();
    let usage = source.disk_usage.as_mut().unwrap();
    usage.storage = Some(dashboard_storage());
    usage.unique_bytes = DiskBytes { logical: 1234, allocated: 150_000_000_000 };
    let before = serde_json::to_vec(&source).unwrap();
    let report = html::inventory(&source, 4 * 1024 * 1024).unwrap();
    assert!(report.contains("data-device=\"7\""));
    assert!(report.contains("data-device=\"8\""));
    assert!(report.contains("5.0%"));
    assert!(report.contains("2.5%"));
    assert!(!report.contains("data-capacity-bytes=\"4000000000000\""));
    assert_eq!(serde_json::to_vec(&source).unwrap(), before);
}
```

- [x] Ajouter les cas absence de capacité, `Some(0)`, ratio supérieur à 100 %, `u64::MAX`, sous-schéma inconnu, somme incohérente, noms dangereux et quatre états de couverture. Constater le rouge avec `rtk proxy cargo test --locked --offline --test html_report dashboard`.
- [x] Implémenter le composant depuis le bloc capturé uniquement. Valider sous-schéma, devices uniques, somme des deux champs et identité du volume avant calcul. Une anomalie conserve la liste et le total historique avec une explication. Trier par allocation décroissante puis device ; première carte visible, autres dans un `<details>` natif. Aucun total de capacité ou d'espace disponible entre volumes.
- [x] Placer le dashboard dans l'overview pour inventaire et revue. La revue utilise les `source.items` et `source.disk_usage` complets, jamais les lignes filtrées. Montrer les comptes keep/candidate/verify seulement avec une revue. Séparer la couverture des dossiers de travail et du Git partagé, et conserver le signal de découverte incomplète. Retirer les gros indicateurs redondants « Not established / Not reviewed » du panneau technique, en gardant leurs explications utiles.
- [x] Générer deux meters natifs par volume avec textes visibles et libellés accessibles : non disponible et allocation mesurée. Le premier utilise `capacity.checked_sub(available)`. Convertir les dixièmes en chaîne décimale dans Rust ; borner la valeur de la barre à `1000` dixièmes et conserver le ratio réel dans le texte et `aria-valuetext`. Si disponible est inconnu, seule la jauge d'allocation peut être affichée ; si capacité est inconnue, aucune barre assimilée à zéro. La légende signale le Git partagé, la mesure partielle et l'espace libérable inconnu.
- [x] Ajouter les styles avec les tokens existants, une grille mobile et des retours à la ligne. Aucun calcul JavaScript et aucune modification de `report.js` nécessaires. Vérifier avec `rtk proxy cargo test --locked --offline --test html_report` et `rtk proxy node tests/html-list.cjs` ; adapter seulement les assertions de présentation affectées des scripts de thème/sections.

**Livrable :** dashboard déterministe, valeurs inconnues expliquées et liste ouverte.

### Tâche 5 : rejeu, publication, rendu et livraison

**Files:** modifier `tests/disk_usage.rs`, `tests/test_html_cli.py`, `tests/html_report.rs`, `docs/disk-usage.md`, `CHANGELOG.md` ; créer `tests/html-dashboard.cjs`.

**Interfaces:** réutiliser `fixture()` de `tests/html_report.rs`, `HtmlCliTests.verify_run` et `plan_keeps_captured_disk_sizes_after_files_change_and_reports_missing_measurements`.

- [x] Étendre le rejeu pour comparer tout `storage` après mutation puis suppression des dossiers synthétiques. Comparer le bloc source entier avant/après filtrage et ses digests. Dans le test Python de publication utilisant `--disk-usage`, vérifier les compteurs JSON/HTML et le reçu natif :

```python
storage = data["disk_usage"]["storage"]
self.assertEqual(sum(row["bytes"]["allocated"] for row in storage["devices"]),
                 data["disk_usage"]["unique_bytes"]["allocated"])
for row in storage["devices"]:
    self.assertIn(f'data-device="{row["device"]}"', html)
```

- [x] Tester le budget de publication avec le bloc enrichi : publication intacte ou refus explicite, jamais rapport tronqué. Le test de filtre doit aussi comparer les compteurs de nettoyage du dashboard au source complet.
- [x] Ajouter `dashboard_fixture_export`, test Rust produisant la fixture à deux volumes avec un gap disque. Export facultatif via `WT_SWEEP_DASHBOARD_FIXTURE_DIR`, fichier `report.html` créé avec `create_new(true)`, aucune donnée personnelle. Ce test vérifie la conservation du source avant/après.
- [x] Écrire le script navigateur sur le modèle des scripts existants : thèmes clair/sombre, largeurs `1440`/`390`, JavaScript actif/inactif. Contrôler les libellés accessibles, les autres volumes via `details`, la liste ouverte, l'absence de requêtes réseau, les captures et ces assertions :

```javascript
const dashboard = page.locator('[data-disk-dashboard]');
assert.equal(await dashboard.isVisible(), true);
assert.equal(await dashboard.locator('meter:visible').count(), 2);
assert.equal(await page.locator('#worktree-families').evaluate(e => e.open), true);
assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
const before = await dashboard.innerText();
if (javaScriptEnabled) {
  await page.locator('#search').fill('no-such-dashboard-worktree');
  assert.equal(await dashboard.innerText(), before);
}
```

- [x] Exécuter le parcours synthétique ci-dessous. Résoudre Playwright déjà installé via les dépendances locales si nécessaire, sans installation. Si indisponible, consigner la vérification visuelle non effectuée. Aucune surface de repli ne contourne un refus d'accès au rapport personnel.

```sh
dashboard_dir="$(mktemp -d /private/tmp/wt-sweep-dashboard.XXXXXX)"
WT_SWEEP_DASHBOARD_FIXTURE_DIR="$dashboard_dir" rtk proxy cargo test --locked --offline --test html_report dashboard_fixture_export -- --exact
rtk proxy node tests/html-dashboard.cjs "$dashboard_dir/report.html" "$dashboard_dir/browser"
rtk proxy python3 -B -m unittest discover -s tests -p 'test_html_cli.py' -v
```

- [x] Mettre à jour le guide disque et les contrats avec les formules, « non disponible », les volumes APFS distincts, l'intervalle de capture et les anciens fichiers. Le changelog décrit les comportements vérifiés, sans annoncer un pourcentage historique reconstruit avec les données actuelles.
- [x] Exécuter tous les contrôles requis et conserver leurs codes de sortie :

```sh
rtk proxy cargo fmt --all -- --check
rtk proxy cargo clippy --all-targets --locked --offline -- -D warnings
rtk proxy cargo test --locked --offline
rtk proxy cargo build --locked --offline
rtk proxy python3 -B -m unittest discover -s tests -p 'test_*.py' -v
rtk proxy python3 -B -m unittest discover -s experiments/exclusion -p 'test_*.py' -v
rtk proxy python3 -B scripts/sync-ai.py --check
rtk proxy git diff --check
```

- [x] Faire relire la frontière native et le diff complet indépendamment selon la méthode approuvée. Corriger puis retester les constats. Une suite échouée reste rapportée comme telle, même si un rejeu ciblé réussit.
- [x] Régénérer le rapport personnel depuis le JSON sauvegardé, sans collecte, Git I/O ni modification du source : nouvelle version HTML à nom libre, provenance distincte d'un reçu natif, hash et toutes les lignes comparés avant/après. L'ancien inventaire affichera « capacité non enregistrée ». Une collecte réelle pour obtenir les pourcentages demande un périmètre explicite après livraison, hors du développement.

**Livrable :** artefacts natifs conservés, rejeu hors ligne vérifié, présentation contrôlée et résultats distinguant succès, erreurs et omissions.

## Conservation et retour arrière

Conserver les captures de base et les diffs par tâche hors des fichiers modifiés.
Si un retour arrière devient nécessaire, réconcilier uniquement les hunks de
cette fonctionnalité avec l'état actuel. Préserver les modifications antérieures.
Sans maîtrise des écritures concurrentes, ne pas restaurer automatiquement par
copie ou reset. Un nouveau fichier ne peut être retiré qu'après comparaison à
son dernier état écrit et réconciliation de toute modification concurrente.
Conserver les preuves ; ne supprimer aucun dossier préexistant.

## Revue du plan et méthode

Recommandation : **native**, un intégrateur dans cette session et une revue
indépendante du diff complet. Les cinq tâches partagent le même contrat et
collecteur, ce qui favorise une exécution séquentielle. L'autre méthode emploie
un agent et un reviewer frais par tâche, puis une revue finale. Le user relit
le plan avant le code. La méthode native avec revue indépendante finale est
autorisée par le user le 11 octobre 2026.

## Bilan avant intégration

La reprise sur la base alpha et les vérifications de livraison A1/A2 sont
consignées dans le [registre courant](../../progress.md#dashboard-et-ci-automatique-11102026).
Les résultats ci-dessous décrivent la première exécution locale.

Les cinq tâches sont réalisées dans le checkout initial, sans commit ni push.
La revue indépendante (gpt-6-astra) a relevé deux points Important et un Minor :
contrôle du budget entre chaque appel natif, découverte interrompue et omissions
près des compteurs, et limite exacte du HTML enrichi. Les trois sont corrigés ;
les deux comportements Important ont été reproduits en échec puis vérifiés.

Validation observée : fmt, clippy, build, 447 tests Rust (3 ignorés), six tests
d'exclusion, synchronisation des 22 fichiers et diff sans erreur. Les 26 tests
HTML passent, ainsi que les huit variantes du dashboard, huit variantes de
sections et douze variantes de thèmes et préférences. Le contrôle du focus
exige au moins 2px, avec couleur et visibilité, plutôt qu'une largeur calculée
exacte : le navigateur a retourné 3px dans une variante mobile.

La suite Python complète a exécuté 316 tests : 307 succès, sept ignorés et trois
échecs de sous-cas répartis entre deux tests. Ces échecs proviennent du sélecteur
de processus indisponible dans le bac à sable (`pgrep`, `sysmond`). Les deux
tests passent après rejeu avec l'accès hôte autorisé. Le troisième scénario de
sélection de processus, ignoré dans la suite, passe également après rejeu. La
suite complète initiale reste enregistrée comme échouée ; les replays sont des
preuves séparées. Quatre fixtures historiques et deux chemins non UTF-8 restent
non vérifiés dans cette suite.

Le rapport personnel V5 a été régénéré depuis le JSON sauvegardé : toutes les
1 466 inscriptions Git sont identiques au source et à V4, liste ouverte et hash
d'inventaire inchangé. La capacité ancienne n'est pas reconstruite. Aucun scan
global ni nettoyage réel n'a été lancé. Le rapport personnel n'a pas été
contrôlé visuellement dans le navigateur ; les contrôles visuels utilisent
uniquement une fixture synthétique. La provenance V5 est distincte d'un reçu
natif de publication.

Les preuves locales sont conservées dans
`.superpowers/sdd/2026-10-11-disk-dashboard/`, notamment `progress.md`,
`final-review.md`, `validation.md`, les captures, les codes de sortie et les
logs RED/GREEN. Ces preuves ne sont pas commitées. Le premier lancement complet
Rust a été interrompu avant correction puis relancé intégralement avec succès.
