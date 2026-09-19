# task.md — Serielle Implementierungs- und Testschritte (01_more → 04_more_lisp)

Jeder Schritt endet mit Validierung; erst bei Grün weiter. Scope-Entscheidungen
(2026-09-19, s. `plan.md`): nur `01_more` transpilieren nach `04_more_lisp/`,
`gen.lisp` + `lisp/`-Module sind Source of Truth, `Cargo.toml` handgeschrieben,
`lprint`-Logging mit C++-Semantik, faktorisierte Fehler-Helper, CL-Nähe vor
Texttreue, `01_more`-Verhalten (`{:.2}`, leere Dateien zählen) geht vor
`03`-Gewohnheit, kein `notify`-Upgrade, `04` in Release-CI, `prompt.txt`
committen, Transpiler-Lücken im Walkthrough mit Beispiel vorschlagen.

## Schritt 0 — Baseline sichern (keine Codeänderung)

1. `cd 01_more && cargo build` → ✅ grün (Referenz baut).
2. `sbcl --eval '(ql:register-local-projects)' --eval '(ql:quickload "cl-rust-generator")'`
   → ✅ lädt (ohne `register-local-projects` → `SYSTEM-NOT-FOUND`, bekannter Befund).
3. `sh /workspace/src/cl-rust-generator/run-tests.sh` → ✅ grün (Ausgangsbasis 173/173).
4. `git status --short` prüfen; `target/`-Verzeichnisse nie stagen.
5. Backup-Disziplin: vor jeder `gen.lisp`-Änderung known-good-Kopie
   (`cp gen.lisp gen.lisp.good` bzw. `lisp/<mod>.lisp`); bei Klammerfehlern
   zurückkehren statt flicken.

## Schritt 1 — Risiko-Spikes (nur `/tmp`, nichts committen)

Je ein minimaler `write-source`-Lauf nach `/tmp`, Ergebnis prüfen:

1. `#[cfg(target_os = "linux")]`-Arme via `attr` (Watcher-Weiche, HUD-Texte).
2. `pub fn` via `space` (alles `pub` in `01_more`: Structs, Fns, Testsichtbarkeit).
3. Typstrings `Option<RecommendedWatcher>`, `Receiver<notify::Result<Event>>`,
   `HashSet<PathBuf>`, `Arc<AtomicBool>`, `&[&OsStr]`, `&mut [FileNode]`.
4. Lifetime-Signatur `find_hovered_path<'a>` (String-Hatch vs. Index-Umbau entscheiden).
5. `matches!(event.kind, EventKind::Access(_))` (Pattern mit `_`-Bindung).
6. `(dot (mouse_wheel) 1)`-Tupelzugriff, `writeln!`-Broken-Pipe (`print_line`),
   `move`-Closure (bekannt aus `03`), Tupel-`let` für `(tx, rx)` (bekannt aus `03`).
7. ✅ Jeder Spike: `sbcl`-Exit 0 + Fragment per `rustc --crate-type lib` oder
   Sichtprüfung plausibel. Befunde + Lücken mit Beispiel notieren
   (Vorlage für Walkthrough-Vorschläge).

## Schritt 2 — Gerüst `04_more_lisp/`

1. `04_more_lisp/Cargo.toml` per Hand: `macroquad 0.4`, `notify 6.1`
   (`default-features = false`, `macos_kqueue`) wie Referenz; Newest-Check s. `deps.md`.
2. `gen.lisp`-Loader (`gen0.lisp`-Muster) + `lisp/helpers.lisp`
   (`lprint`, `ext-is`, `report-skip-fn` aus `03` übernehmen).
3. Erster Generierungslauf `sbcl --load 04_more_lisp/gen.lisp --quit` → ✅ Exit 0.
4. ✅ `cd 04_more_lisp && cargo build` grün (Gerüst, ggf. noch lückenhaft — OK).
5. ✅ `parenmedic diagnose 04_more_lisp/gen.lisp 04_more_lisp/lisp/` sauber.

## Schritt 3 — Module seriell (je Modul: emittieren → generieren → parenmedic → build → fmt)

Reihenfolge (Abhängigkeiten von unten nach oben):

1. `lisp/data.lisp`: `FileNode`-Struct (alle 11 Felder), `impl` (`new`, `find_mut`,
   `update_file_size`); `LayoutWorkspace`; `DirectoryBatch`/`ScanEvent`
   (`defenum`-Spike: Variante mit Daten → ggf. Struct-Hatch).
2. `lisp/layout.lisp`: `worst_aspect_ratio`, `layout_row`, `squarify_children`,
   `collapse_descendants`, `layout_treemap_sequential`, `sum_tree_stats`.
3. `lisp/scan.lisp`: `is_virtual_or_special_fs` (cfg-Hatch), `scan_directory_recursive`,
   `merge_scan_batch`, `merge_scanned_node` (+ `report_skip`-Aufrufe).
4. `lisp/watcher.lisp`: `WatcherManager`-Struct, `new`, `register_dir` (cfg-Weiche,
   Limit-Meldungen).
5. `lisp/render.lisp`: `update_animations`, `snap_tree`, `draw_cushion_rect`,
   `render_treemap`, `find_hovered_path` (Spike-Form aus Schritt 1).
6. `lisp/format.lisp`: `truncate_label`, `get_color_for_filename` (Splice-Farbgruppen),
   `print_line`, `format_bytes` (`{:.2}`!), `window_conf`.
7. `lisp/headless.lisp`: `target_from_args`, `resolve_target`,
   `has_graphical_display` (cfg), `run_headless`.
8. `lisp/main.lisp`: `main` (Headless-Weiche) + `async gui_main` (Drains, Debounce,
   Kamera, HUD); `lprint`-Stellen Scan-Summary/Layout/Watcher.
9. ✅ Nach jedem Modul: Generierung + `cargo build` + `cargo fmt` (Diff zum
   Vorstand lesen, informativ) + `cargo clippy --all-targets -- -D warnings`.

## Schritt 4 — Tests & Äquivalenz

1. Generierte Unit-Tests in `main.rs` (`#[test]`, Muster `03`): `truncate_label`
   (ASCII/CJK/Emoji), Layout (Sortierung/Fläche/Overlap), `target_from_args`,
   `merge_scan_batch`, Scanner-Fixture — aus `01_more`-`mod tests` abgeleitet.
2. `04_more_lisp/tests/headless_scan.rs`: Port von `01_more` (CJK-Fixture mit
   `180`-Total, Broken-Pipe-Fall), Binärname angepasst.
3. ✅ `cd 04_more_lisp && cargo test` grün.
4. ✅ `sh /workspace/src/cl-rust-generator/run-tests.sh` grün (keine Regression).
5. ✅ `diff 01_more/src/main.rs 04_more_lisp/src/main.rs` gelesen und im
   Walkthrough begründet (nur CL-Umformulierung/Helper/Float-Druck/Hatches).
6. ✅ Headless-Fixture manuell: `--headless <dir>` → Summary + `3 files` + `180`.
7. ✅ `lprint`-stderr-Prüfung im Lauf (Scan-/Layout-/Watcher-Zeilen sichtbar).
8. ✅ GUI-Smoke beider Binaries (`xvfb-run -a timeout 20 ...`) → beide Exit 124,
   kein Panic auf stderr (Muster `02`-Walkthrough; `xvfb` ggf. erst installieren).

## Schritt 5 — CI: `04` in Release-Build

1. `.github/workflows/build.yml`: Matrix-Eintrag (`dir: 04_more_lisp`,
   `name: treemap-more-lisp`, `bin: treemap_more_lisp`) + `smoke`-Job um `04`
   (`fmt`/`clippy`/`test`, Headless- + GUI-Smoke) + `rust-cache`-Workspaces ergänzen.
2. ✅ `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/build.yml'))"`.
3. ✅ `cd 04_more_lisp && cargo build --release` grün (GH-Release selbst läuft
   erst bei `v*`-Tag).

## Schritt 6 — Dokumente + Commits (Conventional Commits, nur eigene Pfade!)

1. `task.md` (diese Datei), `deps.md` finalisieren.
2. Commits in Reihenfolge (Format s. `plan.md`, Abschnitt Commit-Konvention):
   1. `docs(plan): add 04_more plan, tasks and deps` (+ `prompt.txt`),
   2. `feat(04_more_lisp): ...` (Generator + Generat, ggf. pro Modul gestapelt),
   3. `test(04_more_lisp): port headless integration tests` (falls separat),
   4. `ci: build and release 04_more_lisp` (Schritt 5),
   5. danach `walkthrough.md` schreiben (Spikes, Hatches, Transpiler-
      Lückenvorschläge mit Beispielen, umgesetzte Verbesserungen, Learnings,
      Erweiterungen, Docker-Programme) und als `docs(plan): add 04 walkthrough`
      committen.
3. ✅ `git status --short` zeigt nur beabsichtigte Dateien (`target/` nie dabei);
   `git log --oneline` zeigt die Commit-Reihe.
