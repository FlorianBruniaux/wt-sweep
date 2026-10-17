# Local Content Comparison Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The human must approve this plan and select or confirm the execution method before implementation. Do not launch subagents merely because they are available.

**Goal:** Montrer, avec des preuves déterministes, quels changements locaux sont déjà présents dans une référence choisie, lesquels diffèrent et lesquels restent non comparés.

**Architecture:** Ajouter une projection facultative aux lectures locales existantes, avec un jugement pur par couche et par chemin. Publier un artefact séparé lié à la revue finale, consommé par les rendus et la compétence assistant. Garder le moteur `triage/1` et ses conditions C1-C5.

**Tech Stack:** Rust 1.95.0, bibliothèque standard, serde/serde_json, sha2, sha1-checked, rustix et libc déjà présents ; Git contrôlé ; Python 3.11 ou supérieur pour les oracles indépendants. Aucune nouvelle dépendance prévue.

**Spec:** [Spécification approuvée](../specs/2026-10-13-local-content-comparison-design.md), validée par l'utilisateur le 13 octobre 2026. Les API et les noms de tests ci-dessous sont proposés pour l'implémentation ; ils ne sont pas encore disponibles.

## Global Constraints

- Garder `review/2`, `triage/1`, `keep`, `verify`, `cleanup_candidate` et C1-C5.
- `apply` reste indisponible ; aucun retrait, fetch, prune, gc ou suppression de branche sur des repos réels.
- Utiliser seulement les cibles d'intégration explicites et le checkout de référence explicitement choisi dans la même famille.
- Lire index et disque séparément ; conserver ignorés, cachés, conflits et inconnues.
- Conserver `BytePath`, OID complets, intervalles, identités, limites et couverture ; aucune normalisation de contenu.
- Standard : 4 096 observations détaillées ; thorough : 32 768. Les lectures de référence consomment les plafonds locaux existants.
- Comparer contenu/type/mode annoncé ; ne pas revendiquer une équivalence d'ACL, attributs étendus, propriétaire/groupe ou restauration.
- Ne pas publier les octets de fichiers ni les valeurs de configuration. Les fixtures restent synthétiques.
- Ne pas modifier une source canonique du harness par sa projection générée ; générer les deux projections après édition canonique.
- Préserver les changements préexistants, notamment disque/HTML. L'intégrateur possède les contrats, Cargo, CLI et publication.
- Les commits, pushes et publications demandent l'autorisation correspondante. Les checkpoints ci-dessous ne font pas de commit implicite.

## Review Focus

1. Chemin relatif contenant `..`, absolu ou passant par un symlink intermédiaire : refuser la lecture hors du checkout de référence. Test LC3.
2. Une famille comprend le checkout de référence parmi les lignes source : ne pas le comparer à lui-même et ne pas le masquer. Test LC5.
3. Un fichier absent apparaît pendant le contrôle de son répertoire : invalider le témoin d'absence. Test LC3.
4. Couche index différente et couche disque inconnue sur le même chemin : garder la différence positive, une seule entrée de chemin et une couverture partielle. Test LC1.
5. Un filtre sélectionne une seule ligne et un bilan d'une autre revue porte les mêmes noms de branches : refuser la liaison ; garder les totaux source de la revue correcte. Tests LC4/LC6.

## Carte des fichiers

| Unité | Fichiers nouveaux | Raccordements existants |
| --- | --- | --- |
| LC1 : types et règles | `src/comparison.rs`, `tests/comparison.rs` | `src/lib.rs` |
| LC2 : faits source | Tests dans `src/local/summary.rs` et `tests/local_comparison.rs` | `src/local/summary.rs`, `src/local.rs` |
| LC3 : références contrôlées | `src/local/comparison.rs` | Lectures et cache de `src/local/summary.rs`, contrôles natifs existants |
| LC4 : artefact et publication | `tests/comparison_run.rs` | `src/comparison.rs`, `src/run.rs`, `src/output.rs`, `src/report_access.rs` si nécessaire pour la lecture liée |
| LC5 : orchestration | `tests/test_local_comparison.py` | `src/review.rs`, `src/model.rs` uniquement pour `Observations` non sérialisé, `src/main.rs` |
| LC6 : rendu | `src/presentation/comparison.rs`, `tests/comparison_presentation.rs`, `tests/html-comparison.cjs` | `src/presentation.rs`, `src/presentation/console.rs`, `src/presentation/html.rs`, CSS/JS uniquement pour les interactions nécessaires |
| LC7 : guides et compétence | Aucun fichier de configuration globale | `docs/contracts.md`, `docs/cli.md`, `docs/html-report-plan.md`, `docs/assistant-skill.md`, `harness/skills/wt-sweep`, projections générées, tests de skill, `CHANGELOG.md` |
| LC8 : recette intégrée | Compléments aux tests précédents | Oracles indépendants existants, registre de validation |

Les lots sont séquentiels : LC1 → LC2 → LC3 → LC4 → LC5 → LC6 → LC7 → LC8. LC2 et LC3 partagent le collecteur et son budget ; ne pas les confier à des exécutants simultanés sur les mêmes fichiers.

### LC0 : établir les préimages et le socle de validation

**Fichiers :** aucun code produit à modifier ; copies et journal dans un dossier temporaire possédé.

- [ ] Lire la spécification, `AGENTS.md`, les contrats, les profils locaux et les frontières natives. Comparer le HEAD et les chemins sales à l'état actuel avant toute édition.

```sh
rtk proxy git status --short
rtk proxy git rev-parse HEAD
python3.11 -c 'import sys; assert sys.version_info >= (3, 11)'
```

- [ ] Enregistrer les préimages des seuls fichiers à modifier, ainsi que les fichiers HTML non suivis dont ils dépendent. Ce script d'exécution sauvegarde les contenus sans les publier ni faire de commit.

```python
import hashlib, json, shutil, tempfile
from pathlib import Path
root = Path.cwd()
backup = Path(tempfile.mkdtemp(prefix="wt-sweep-comparison-preimages-", dir="/private/tmp"))
paths = [
    "src/lib.rs", "src/model.rs", "src/local.rs", "src/local/summary.rs",
    "src/review.rs", "src/main.rs", "src/run.rs", "src/output.rs",
    "src/report_access.rs", "src/presentation.rs", "src/presentation/console.rs",
    "src/presentation/html.rs", "src/presentation/html/report.css",
    "src/presentation/html/report.js", "src/presentation/html/dashboard.rs",
    "src/presentation/html/help.rs", "docs/contracts.md", "docs/cli.md",
    "docs/html-report-plan.md", "docs/assistant-skill.md", "CHANGELOG.md",
]
paths += [str(p.relative_to(root)) for p in sorted((root / "harness/skills/wt-sweep").rglob("*")) if p.is_file()]
manifest = {}
for rel in paths:
    source = root / rel
    if not source.exists():
        manifest[rel] = None
        continue
    assert source.is_file() and not source.is_symlink(), rel
    target = backup / rel
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)
    manifest[rel] = hashlib.sha256(source.read_bytes()).hexdigest()
(backup / "preimages.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(backup)
```

- [ ] Relever les préimages des projections générées et des tests existants avant leur lot, de la même manière. Si une préimage change pendant le travail, réconcilier les changements au lieu de restaurer un ancien fichier complet.
- [ ] Exécuter les contrôles obligatoires du bloc LC8 une première fois. Garder les échecs préexistants avec leur commande et sortie ; ne pas les attribuer au nouveau lot ni déclarer le socle vert par défaut.

**Sortie :** baseline et sauvegarde locale identifiées ; aucun reset, stash ou nouveau commit de base. L'exécution native peut garder le checkout courant avec des hunks ciblés. Si une isolation est choisie, préparer un checkout par le workflow de worktree et y conserver explicitement les préimages nécessaires ; ne pas supposer que les changements non committés ont été copiés.

### LC1 : types, comparaison pure et agrégation

**Fichiers :** créer `src/comparison.rs`, exporter dans `src/lib.rs`, créer `tests/comparison.rs`.

**Interfaces produites :** les types et fonctions suivants vivent dans `wt_sweep::comparison`. Implémenter les derives de sérialisation et `snake_case` pour les enums sérialisés. `FileKind` n'accepte que fichier régulier et symlink ; les autres cas deviennent `Unknown` avant le jugement.

```rust
pub enum FileKind { Regular, Symlink }
pub enum Layer { Index, Disk, Deletion, Additional }
pub enum Origin { Git, Checkout }
pub enum ComparisonResult { MatchesTarget, MatchesReference, Different, Unknown }
pub enum Fingerprint {
    Absent,
    Present {
        kind: FileKind,
        git_mode: u32,
        posix_mode: Option<u32>,
        sha256: String,
    },
    Unknown { reason: String },
}
pub struct ReferenceFingerprint {
    pub capture_id: String,
    pub origin: Origin,
    pub fingerprint: Fingerprint,
}
pub struct RowEvidence {
    pub result: ComparisonResult,
    pub matched_references: Vec<String>,
    pub unknown_references: Vec<String>,
}
pub struct PendingChange {
    pub path: wt_sweep::model::BytePath,
    pub layer: Layer,
    pub stage: Option<u8>,
    pub fingerprint: Fingerprint,
    pub source_oid: Option<String>,
}
pub struct ComparisonRow {
    pub change: PendingChange,
    pub evidence: RowEvidence,
}
pub struct ReferenceCapture {
    pub id: String,
    pub origin: Origin,
    pub path_or_ref: crate::model::BytePath,
    pub identity: Option<crate::model::Identity>,
    pub target_oid: Option<String>,
    pub tree_oid: Option<String>,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub checks: String,
    pub gaps: Vec<crate::model::Gap>,
}
pub struct ComparisonRequest {
    pub captures: Vec<ReferenceCapture>,
    pub checkout: Option<crate::model::BytePath>,
    pub max_rows: usize,
}
pub struct ResultTotals {
    pub matches_target: usize,
    pub matches_reference: usize,
    pub different: usize,
    pub unknown: usize,
}
pub struct WorktreeComparisonDraft {
    pub path: crate::model::BytePath,
    pub worktree_identity: Option<crate::model::Identity>,
    pub git_dir: Option<crate::model::BytePath>,
    pub head: Option<String>,
    pub references: Vec<ReferenceCapture>,
    pub rows: Vec<ComparisonRow>,
    pub path_totals: ResultTotals,
    pub layer_totals: ResultTotals,
    pub complete: bool,
    pub omitted_rows: usize,
    pub gaps: Vec<crate::model::Gap>,
}
pub struct ComparisonDraft {
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub limits: std::collections::BTreeMap<String, u64>,
    pub items: Vec<WorktreeComparisonDraft>,
}
pub fn classify(source: &Fingerprint, references: &[ReferenceFingerprint]) -> RowEvidence;
pub fn path_result(rows: &[ComparisonRow]) -> ComparisonResult;
```

- [ ] Ajouter un test rouge de suppression contre fichier vide. Le helper `present` ci-dessous est défini dans le fichier de test et réutilisé explicitement par ses cas.

```rust
use wt_sweep::comparison::*;
fn present(bytes: &[u8]) -> Fingerprint {
    Fingerprint::Present {
        kind: FileKind::Regular,
        git_mode: 0o100644,
        posix_mode: Some(0o644),
        sha256: wt_sweep::output::sha256(bytes),
    }
}
#[test]
fn deletion_does_not_match_empty_file() {
    let refs = vec![ReferenceFingerprint {
        capture_id: "target-main".into(), origin: Origin::Git,
        fingerprint: present(b""),
    }];
    assert_eq!(classify(&Fingerprint::Absent, &refs).result, ComparisonResult::Different);
}
```

- [ ] Lancer `rtk proxy cargo test --locked --offline --test comparison`. Avant le code, constater l'échec attendu sur le module/API absent ; après le code, constater le résultat du comportement.
- [ ] Implémenter le jugement avec cette règle : source inconnue ou aucune référence → inconnu ; égalité positive → résultat de la meilleure origine prouvée et liste de références inconnues conservée ; aucune égalité avec au moins une référence inconnue → inconnu ; sinon → différent. Comparer mode Git aux cibles Git et mode POSIX connu aux copies locales. Une couche index sans mode POSIX connu ne prouve pas une égalité POSIX avec une copie locale.
- [ ] Ajouter des tests pour un octet différent, un mode différent, symlink différent, absence/absence, référence inconnue avec égalité positive et source inconnue. Ajouter l'agrégation par chemin : `different` prioritaire, puis `unknown`, puis toutes couches cibles Git, sinon copie locale. Compter les inconnues par couche même si le chemin est déjà différent.
- [ ] Ajouter un test où une preuve `matches_target` et une référence inconnue gardent le résultat positif, mais rendent la couverture globale partielle. Ajouter le cas Review Focus 4 sans compter le chemin deux fois.

**Sortie :** tests positifs et contre-exemples verts ; aucun nom de fichier ou état de PR dans `classify`.

### LC2 : projeter tous les changements source pendant la lecture locale

**Fichiers :** `src/local/summary.rs`, `src/local.rs`, créer `tests/local_comparison.rs`.

**Consomme :** `PendingChange`, `Fingerprint`, `Layer` de LC1.

**Produit :** une collecte facultative dont les points d'entrée existants gardent leur comportement par défaut. Utiliser `ComparisonRequest` défini dans LC1, puis les captures validées de LC3 lors du raccordement. La projection source seule peut employer une liste vide pour son test interne et produire des résultats inconnus, sans offrir cette liste vide à la CLI.

```rust
pub(crate) fn collect_summary_for_review_with_comparison(
    worktree: &crate::model::Worktree,
    limits: &crate::local::SummaryLimits,
    request: Option<&crate::comparison::ComparisonRequest>,
) -> Result<crate::local::SummaryOutcome, crate::model::SweepError>;
```

`SummaryOutcome` gagne `comparison: Option<WorktreeComparisonDraft>` ; utiliser le type défini dans LC1 pour les changements détaillés, les compteurs de chemins/couches, le statut de collecte et les lacunes. Ce draft ne contient pas de digest de revue finale.

- [ ] Écrire un test rouge avec plus de huit chemins supplémentaires distincts. Vérifier que la projection voit les changements réellement parcourus et non seulement `LocalCount.samples`. Ajouter index modifié + disque remis à HEAD, puis intent-to-add et conflit.
- [ ] Exécuter `rtk proxy cargo test --locked --offline --test local_comparison` et relever l'échec attendu.
- [ ] Ajouter la projection là où index/HEAD/disque sont effectivement comparés et où les fichiers supplémentaires sont lus. Préserver le hash d'un fichier ignoré au lieu de le recalculer dans un deuxième parcours. Émettre séparément les stages d'index et le disque ; les conflits sont `Unknown` avec stages conservés.
- [ ] Borner la projection à `max_rows`. Dès que le détail ne peut plus être conservé, incrémenter les omissions, garder les faits positifs déjà observés et marquer la couverture incomplète. Ne pas reconstruire des chemins omis depuis les échantillons.
- [ ] Tester la projection désactivée, le cap à deux observations, les budgets d'octets/temps et les erreurs natives. Comparer les compteurs locaux préexistants sur des fixtures stables sous les plafonds. Réutiliser `LocalHooks` pour fautes/dataless, jamais pour simuler un Git positif.

**Sortie :** mêmes lectures de source et mêmes règles locales ; données de comparaison bornées, avec inconnues et omissions observables.

### LC3 : lire les références avec le même budget et les mêmes frontières

**Fichiers :** créer `src/local/comparison.rs`, raccorder `src/local.rs` et `src/local/summary.rs`; compléter `tests/local_comparison.rs`.

**Consomme :** `ReferenceCapture` et `ComparisonRequest` définis dans LC1, changements de LC2 et empreintes de LC1. Ajouter seulement le contrat I/O interne :

```rust
pub(crate) trait ComparisonIo {
    fn lookup(
        &mut self, reference: &ReferenceCapture,
        path: &crate::model::BytePath, layer: Layer,
    ) -> Result<Fingerprint, crate::model::SweepError>;
}
```

**Produit :** lectures positives via le `Collector` existant et sessions Git contrôlées. `lookup` partage son budget, ses caches et son échéance avec ce collecteur ; aucun lecteur autonome à budget illimité.

- [ ] Ajouter le positif « source modifiée avec contenu identique à un blob de la cible courante ». Construire le blob et son commit dans un repo temporaire SHA-1, puis SHA-256, avec les vrais objets Git. Ajouter son contre-exemple d'un octet.
- [ ] Lire commits/arbres/blobs par `ControlledGit` et leur vérification existante. Chercher le même chemin relatif dans l'arbre capturé ; une absence n'est prouvée qu'après lecture complète des sous-arbres nécessaires. Cache par famille physique, OID et format d'objet dans le même run.
- [ ] Lire le checkout fourni par descripteur et composants relatifs validés. Refuser chemins absolus, `..`, `.git` et traversée de symlink intermédiaire ; comparer le lien final par `readlink`, sans suivre sa cible. Recontrôler le répertoire parent avant d'utiliser un témoin d'absence.
- [ ] Ajouter Review Focus 1 et 3, les changements de permissions/type, les repos imbriqués, gitlinks, dataless et fichiers spéciaux. Chaque refus fournit une lacune avec sa provenance.
- [ ] Ajouter un témoin d'écriture pendant lecture à taille/mtime identiques, puis une ref modifiée après cache. La revalidation retire les égalités affectées et garde les inconnues. Une cible Git illisible n'est pas supprimée de la liste demandée.
- [ ] Refaire les tests LC2 et LC3, puis `rtk proxy cargo test --locked --offline --test local_summary`. Vérifier que les lectures de référence comptent dans les plafonds source, y compris les blobs sur cache vérifié et les fichiers ignorés.

**Sortie :** preuve par référence nommée ; aucune égalité calculée à partir d'un titre de PR, d'un patch-id ou d'un nom de cache.

### LC4 : artefact lié, validation et publication

**Fichiers :** `src/comparison.rs`, `src/run.rs`, `src/output.rs`, `src/report_access.rs` si la lecture liée l'utilise ; créer `tests/comparison_run.rs`.

**Consomme :** contrat `ComparisonDraft` de LC1, projection de LC2/LC3 et revue finale vérifiée. Les tests de publication construisent leurs données synthétiques sans attendre le raccordement CLI de LC5.

**Produit :** `LocalComparison` sérialisé avec ces clés racines : `schema`, `inventory_sha256`, `review_sha256`, `digest_sha256`, `started_unix_ms`, `finished_unix_ms`, `limits`, `items`. Chaque item porte `path`, `worktree_identity`, `git_dir`, `head`, `references`, `rows`, `path_totals`, `layer_totals`, `complete`, `omitted_rows`, `gaps`. Les références utilisent `ReferenceCapture`; les lignes utilisent `ComparisonRow`.

```rust
pub fn seal(draft: ComparisonDraft, review: &crate::model::Review)
    -> Result<LocalComparison, crate::model::SweepError>;
pub fn verify(document: &LocalComparison, review: &crate::model::Review)
    -> Result<(), crate::model::SweepError>;
```

`seal` valide les identités, ordonne les entrées de façon stable, fixe `schema` à `wt-sweep.local-comparison/1` et lie la revue finale. `verify` recalcule les digests et les compteurs, vérifie les chemins uniques, les captures nommées, OID, intervalles et complétude ; il ne se contente pas de comparer deux noms de branche.

- [ ] Écrire les tests rouges : modification d'une ligne sans changement de digest ; mauvais digest de revue ; chemin dupliqué ou étranger ; référence inexistante dans une preuve ; entier de compteur incohérent. Ajouter Review Focus 5 sur un filtre sélectionnant une ligne.
- [ ] Créer `LocalComparison` avec les champs ci-dessus et réutiliser les drafts typés de LC1 avant de raccorder leur sérialisation. Les contrôles de contenu sont dans `verify` même pour une valeur construite par un autre appel Rust.
- [ ] Ajouter un nom d'artefact autorisé exact `local-comparison.json` à `RunDirectory::open_artifact`. Introduire un argument optionnel de comparaison aux nouvelles fonctions de publication, en gardant les wrappers publics actuels sans comparaison. Ne pas autoriser un nom arbitraire fourni par le JSON.
- [ ] Définir le raccordement de publication ci-dessous et son variant `publish_review_with_comparison_io` avec un argument final `io: &mut impl PublicationIo`, pour les injections. Parser et vérifier la source de `artifact.json` (revue ou vue de revue), puis appeler `verify` avec cette source avant toute écriture. Ne pas accepter un document validé contre une revue différente de celle réellement publiée.

```rust
pub fn publish_review_with_comparison(
    base: &std::path::Path,
    artifact: crate::run::Artifact<'_>,
    boundaries: &crate::model::Inventory,
    inventory_json: Option<&[u8]>,
    comparison: Option<&crate::comparison::LocalComparison>,
) -> Result<std::path::PathBuf, crate::model::SweepError>;
```

- [ ] Écrire et synchroniser l'artefact avant le reçu, enregistrer son SHA-256 et sa taille. Ajouter phases `ComparisonWrite` et `ComparisonSync` pour l'injection des fautes. Utiliser les limites de publication existantes et refuser tout dépassement avant d'annoncer un run complet.
- [ ] Ajouter les fautes écriture/sync et répertoire remplacé, sortie dans le checkout de référence, symlink d'artefact et détail dépassant le plafond. Le reçu ne peut pas certifier une comparaison manquante. Vérifier un ancien run sans artefact avec le rendu « non collectée ».
- [ ] Lancer `rtk proxy cargo test --locked --offline --test comparison_run` puis `rtk proxy cargo test --locked --offline --test run --test output`.

**Sortie :** comparaison traçable à la revue, publication bornée et défauts visibles ; anciens consommateurs de `review/2` inchangés.

### LC5 : options, orchestration et maintien du verdict

**Fichiers :** `src/review.rs`, `src/model.rs` pour `Observations`, `src/main.rs`; créer `tests/test_local_comparison.py`.

**Consomme :** captures LC3, projection LC2, règles LC1.

**Produit :** champ non sérialisé `Observations.comparison: Option<ComparisonDraft>`. Utiliser `ComparisonDraft` défini dans LC1 pour regrouper les drafts par worktree et les intervalles du run. `ReviewOptions` gagne `comparison: ComparisonOptions`, type défini dans `src/comparison.rs` avec `enabled: bool`, `reference: Option<BytePath>` et `max_rows: usize` ; défaut désactivé, 4 096 détails standard, 32 768 thorough.

- [ ] Créer l'oracle CLI sur `HistoryFixture`, sans importer le jugement Rust. Le cas rouge ci-dessous devient un test de la classe `LocalComparisonTests` du nouveau fichier.

```python
import json, os, subprocess, tempfile, unittest
from pathlib import Path
from inventory_support import BINARY, HistoryFixture, decode_path

class LocalComparisonTests(unittest.TestCase):
    def test_ignored_duplicate_and_unique_content(self):
        fixture = HistoryFixture(files={"tracked.txt": b"base\n", ".gitignore": b"node_modules/\n"})
        self.addCleanup(fixture.close)
        output = tempfile.TemporaryDirectory(prefix="wt-sweep-comparison-run-", dir="/private/tmp")
        self.addCleanup(output.cleanup)
        for root in (fixture.principal, fixture.linked):
            (root / "node_modules").mkdir()
            (root / "node_modules/copy").write_bytes(b"same\n")
        (fixture.linked / "node_modules/unique").write_bytes(b"private fixture work\n")
        result = subprocess.run([
            str(BINARY), "review", "--repo", str(fixture.principal),
            "--target", "refs/heads/main", "--worktree", str(fixture.linked),
            "--compare-local", "--compare-reference", str(fixture.principal),
            "--run-dir", output.name,
        ], env=fixture.env, capture_output=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stderr)
        report = next(Path(output.name).glob("run-*/local-comparison.json"))
        data = json.loads(report.read_text())
        item = next(x for x in data["items"] if decode_path(x["path"]) == os.fsencode(fixture.linked))
        outcomes = {decode_path(x["change"]["path"]): x["evidence"]["result"] for x in item["rows"]}
        self.assertEqual(outcomes[b"node_modules/copy"], "matches_reference")
        self.assertEqual(outcomes[b"node_modules/unique"], "different")
```

- [ ] Lancer `python3.11 -B -m unittest discover -s tests -p 'test_local_comparison.py' -v`. L'échec initial attendu est le refus des options non implémentées.
- [ ] Ajouter `--compare-local` et `--compare-reference`. Exiger un run de publication ; refuser `--out` mono-fichier avec comparaison. Si aucune référence n'est fournie, refuser avec une aide lisible. Ne pas activer la comparaison par défaut ni lancer de scan global.
- [ ] Lier les captures à `FamilyTargets` avant la phase locale, puis utiliser le point d'entrée LC2. Ne modifier aucun champ sérialisé de `ReviewItem` ou `Review`, ni les fonctions C1-C5. Récupérer les drafts parallèlement aux résumés et les revalider avant le digest final.
- [ ] Valider le checkout explicitement fourni dans la même famille. Un checkout parmi les lignes source reçoit une comparaison non collectée pour lui-même, avec sa raison ; un `--worktree` qui sélectionne uniquement ce checkout est refusé. Plusieurs familles avec une référence unique ambiguë sont refusées. Ajouter ces deux cas de Review Focus 2.
- [ ] Ajouter fixtures squash, PR mergée importée avec code supplémentaire, ref intermédiaire non désignée, filtre d'affichage et reprise `--worktree`. Comparer `classification` et `decision` avec/sans comparaison sur un corpus stable sous les plafonds, en supprimant seulement les champs d'intervalle/digest pour les autres faits.

**Sortie :** commandes proposées fonctionnelles sur fixtures, source entière conservée, autres lignes explicitement non comparées, aucun candidat promu par le nouveau bilan.

### LC6 : récapitulatif et détails dans les trois rendus

**Fichiers :** créer `src/presentation/comparison.rs`, `tests/comparison_presentation.rs`, `tests/html-comparison.cjs`; raccorder les rendus texte/console/HTML existants.

**Consomme :** artefact validé LC4 et `ReviewView` source conservée.

**Produit :** wrappers optionnels, sans casser les points d'entrée historiques :

```rust
pub fn review_with_comparison(
    view: &crate::model::ReviewView,
    comparison: Option<&crate::comparison::LocalComparison>, limit: usize,
) -> Result<String, crate::model::SweepError>;
pub fn view_detail_with_comparison(
    view: &crate::model::ReviewView,
    comparison: Option<&crate::comparison::LocalComparison>, limit: usize,
) -> Result<String, crate::model::SweepError>;
```

Le premier vit dans `presentation::html`; le second dans `presentation`. Le module `presentation::comparison` fournit les libellés et les compteurs communs après `verify`. La console utilise la même projection pour le résumé compact. Pour une revue sans vue, créer aussi `detail_with_comparison(review: &Review, comparison: Option<&LocalComparison>, limit: usize) -> Result<String, SweepError>` dans `presentation`; ce wrapper valide la même liaison et garde le rendu historique si la comparaison est absente.

- [ ] Ajouter un test rouge qui attend le titre « Que reste-t-il à récupérer ? », un chemin index différent, un disque correspondant à une cible et la couverture partielle. Vérifier qu'il n'affiche ni une suppression autorisée ni la valeur synthétique d'un secret.
- [ ] Ajouter le bilan par carte et au récapitulatif, puis les références exactes et les couches en détails dépliables. Les quatre compteurs de chemins ne double-comptent pas les couches. Montrer à part les observations inconnues restantes.
- [ ] Conserver le verdict, ses raisons et ses conditions manquantes. Quand tout le contenu local comparé est présent ailleurs, le texte demande de terminer les autres preuves Git/conservation ; il ne dit pas « supprimable » si C1-C5 ne le permettent pas.
- [ ] Relier console, texte et HTML à la même projection validée. Le HTML initial contient les preuves sans JS. La recherche existante indexe les chemins de comparaison affichés, avec des totaux source distincts des lignes filtrées ; ajouter une interaction uniquement si elle est nécessaire à ce raccordement.
- [ ] Ajouter cas non UTF-8, caractères HTML, filtre d'une ligne avec mêmes noms de branche, run sans comparaison, cap HTML minuscule et compteurs différents/inconnus mixtes. Le test `.cjs` vérifie le comportement rendu ; le test Rust vérifie l'échappement et les limites.
- [ ] Lancer `rtk proxy cargo test --locked --offline --test comparison_presentation --test html_report`, puis intégrer `html-comparison.cjs` au runner de `tests/test_html_cli.py` et exécuter ce runner avec Python 3.11.

**Sortie :** même bilan observable dans les trois rendus, avec preuves utilisables et couverture visible.

### LC7 : guides et compétence canonique

**Fichiers :** guides de la carte, compétence `harness/skills/wt-sweep/SKILL.md` et références `commands.md`/`reports.md`, projections générées et tests de workflow/routage.

**Consomme :** options implémentées LC5, artefact LC4 et rendus LC6. Les descriptions documentaires sont vérifiées contre `--help` et un run fixture, pas contre ce plan seul.

- [ ] Charger la compétence `skill-creator` avant son édition et appliquer les règles repo, double projection et routage BM25. Aucun fichier global n'est modifié ; les préimages de la source canonique et des projections sont vérifiées avant génération.
- [ ] Écrire un test rouge de workflow : ancien run autorisé sans comparaison ; nouvel artefact altéré refusé ; bilan interprété avec ses compteurs originaux et sans changement du verdict.
- [ ] Décrire les options, la facturation du budget partagé, les différences d'origine, la référence sale, les limites de mode/métadonnées et le ciblage `--worktree`. Expliquer que « différent » ne signifie pas « utile » et que « identique ailleurs » ne signifie pas « sauvegardé durablement ».
- [ ] Modifier seulement les sources canoniques. Générer puis contrôler :

```sh
python3.11 -B scripts/sync-ai.py --write
python3.11 -B scripts/sync-ai.py --check
python3.11 -B -m unittest discover -s tests -p 'test_skill_workflow.py' -v
python3.11 -B -m unittest discover -s tests -p 'test_harness.py' -v
```

- [ ] Mettre le changelog à jour avec la fonction réellement implémentée et ses limites ; garder les autres entrées locales. Ne pas annoncer une release, une découverte native de compétence ou une activation de hooks depuis les seules validations de fichiers.

**Sortie :** guides et deux projections cohérents, tests de routage/lecture effectués, aucune instruction de retrait automatique.

### LC8 : recette indépendante et validation intégrée

**Fichiers :** compléter `tests/test_local_comparison.py`, tests LC1-LC7 et registre de validation.

- [ ] Couvrir les cas d'acceptation 1 à 14 de la spécification selon la matrice ci-dessous. Les positifs Git sont des repos réels jetables ; les injections servent seulement aux fautes et à la concurrence.
- [ ] Construire les manifestes source/référence avant et après avec `filesystem_manifest` de `tests/inventory_support.py`, hors implémentation Rust. Comparer fichiers, index, refs, objets et marqueurs de hooks ; vérifier séparément que le run n'a pas écrit dans la référence.
- [ ] Exécuter les contrôles obligatoires dans l'ordre, avec Python 3.11 ou supérieur. Lancer les suites lourdes successivement, pas en concurrence.

```sh
rtk proxy cargo fmt --all -- --check
rtk proxy cargo clippy --all-targets --locked --offline -- -D warnings
rtk proxy cargo test --locked --offline
rtk proxy cargo build --locked --offline
python3.11 -B -m unittest discover -s tests -p 'test_*.py' -v
python3.11 -B -m unittest discover -s experiments/exclusion -p 'test_*.py' -v
python3.11 -B scripts/sync-ai.py --check
rtk proxy git diff --check
```

- [ ] Produire un rapport synthétique complet avec doublon ignoré, contenu distinct, conflit et inconnue de référence. Vérifier la liaison artefact/reçu, les compteurs et le rendu HTML ; ne pas scanner la machine pour cette recette.
- [ ] Faire relire le diff final et les preuves par un auteur indépendant après autorisation de ce mode d'exécution. Le relecteur cherche les fausses égalités, les omissions, les budgets contournés et les promotions indues. Corriger les findings, puis rejouer seulement les contrôles affectés avant la clôture.
- [ ] Rapporter les fichiers du lot, commandes observées, échecs préexistants ou nouveaux, inconnues et artefacts. Ne pas transformer un test structurel en preuve d'une interface rendue ni un succès local en succès CI/publication.

| Acceptation de la spécification | Lot et preuve |
| --- | --- |
| 1 : contenu cible identique / un octet différent | LC1 et LC3, objets vérifiés dans SHA-1/SHA-256 |
| 2 : index distinct, disque correspondant | LC2, LC5 et LC6, couches conservées |
| 3 : squash / arbres équivalents | LC5, aucun changement de C2 ni du verdict |
| 4 : fusion importée / travail supplémentaire | LC5, oracle CLI et historique privé |
| 5 : ignoré doublon / ignoré unique | LC3/LC5, oracle `node_modules` |
| 6 : configuration différente / secret synthétique | LC1/LC6/LC8, empreintes différentes et contenu absent des sorties |
| 7 : suppression, vide, modes, liens | LC1/LC3, positifs et contre-exemples |
| 8 : formats Git, objet illisible, noms bruts | LC3/LC4/LC6, OID et BytePath sans perte |
| 9 : mutation, index spécial, types refusés | LC2/LC3/LC5, injections de fautes natives |
| 10 : plusieurs cibles et preuve positive partielle | LC1/LC3/LC6, résultats et lacunes simultanés |
| 11 : plafonds et reprise ciblée | LC2/LC5/LC6, zéros incomplets jamais certifiés |
| 12 : artefact, liaison et publication | LC4, corruption et fautes d'I/O |
| 13 : aucune mutation des cibles | LC8, oracle de manifestes et témoins indépendants |
| 14 : verdicts conservés et analyse utile | LC5/LC8, fixtures stables avec/sans comparaison |

## Conditions d'arrêt et remise

Un objet illisible ou un plafond atteint donne une inconnue explicite. Arrêter le lot concerné si les noms de chemins sont perdus, si une lecture sort des frontières, si une comparaison contourne son budget, si une égalité non vérifiée apparaît, si les conditions C1-C5 sont relâchées ou si une préimage concurrente serait écrasée. Corriger ou rendre le refus observable avant de poursuivre.

Les changements proposés de types sont internes ou dans l'artefact séparé. Si l'exécution découvre qu'il faut modifier le contrat sérialisé `review/2`, ajouter une dépendance, récupérer des données de forge en ligne ou changer la politique de retrait, présenter cette modification de périmètre avant de l'implémenter.

Ce plan ne lance pas l'exécution et n'autorise aucun commit, push, publication ou retrait réel. Après revue et validation humaines, utiliser la méthode choisie et conserver les checkpoints LC0-LC8. L'exécution native dans cette session avec une revue indépendante finale est recommandée : les lots partagent le collecteur, ses budgets et le publisher.
