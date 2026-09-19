## Goal

Den Rust-Code von `01_more` (1440 Zeilen, `treemap-disk-analyzer`: Squarified-Treemap,
Batched-Scanner, `notify`-Live-Watcher, Animation/Kamera/HUD, Headless-Modus) per
`cl-rust-generator`-Transpiler aus Lisp-Quellen nach `04_more_lisp/` überführen.
`gen.lisp` (Loader nach Muster `03_mvp_lisp/gen0.lisp`, Vorbild
`cl-rust-generator/examples/21_mandelbrot/gen00.lisp`) plus kleine
Emitter-Module unter `04_more_lisp/lisp/` sind danach Source of Truth,
`04_more_lisp/src/main.rs` wird per `sbcl`-Lauf generiert und committet.
Verhalten: äquivalent zu `01_more` (beobachtbar: Scan-Ergebnis, Layout,
GUI-Liveness, Headless-Output, Watcher-Verhalten) — CL-Nähe geht vor Texttreue,
begründete Verbesserungen werden gleich umgesetzt. Ergebnis: laufendes
`04_more_lisp/`-Projekt mit Release-CI-Eintrag, Tests und Plan-Dokumenten
(`plan.md`, `task.md`, `deps.md`, `walkthrough.md` in `plan/20260919_01_more/`).

## Success Criteria

- `04_more_lisp/gen.lisp` existiert, folgt dem `gen0.lisp`-Muster (`ql:quickload`
  mit `register-local-projects`, `in-package`, `write-source` mit `` `(do0 ...) ``),
  lädt Emitter-Module aus `lisp/` (jede Emitter-`defun` ≤ 60 Zeilen, Top-Level-Closer
  auf eigenen Zeilen) und generiert per `sbcl`-Lauf ohne Fehler `src/main.rs`.
- Generierter Code baut mit `cargo build`, ist `cargo fmt --check`-sauber und
  `cargo clippy --all-targets -- -D warnings`-frei.
- Verhaltens-Äquivalenz zu `01_more` ist belegt (Diff-Analyse informativ, Headless-Tests,
  GUI-Smoke unter `xvfb-run`; Details s. Validation Plan). Abweichungen sind dokumentiert.
- `lprint`-Logging (C++-Semantik aus `03`, Auto-Stringifizierung, ein `eprintln!`
  pro Zeile) an ≥ 2 Laufzeitstellen (Scan-Summary, Layout-/Watcher-Kennzahlen);
  repetitive Generierung via `,@(loop ...)`-Splices und Lisp-`defun`s faktorisiert.
- Alle Fehlerstellen melden sich (kein stilles `Err`-Schlucken) über genau eine
  generierte Rust-Helper-`fn` pro Bereich (Scan: `report_skip` wie in `03`; Watcher:
  `eprintln!`-Orte wie in der Referenz).
- Unit-Tests (generiert, `#[test]` in `main.rs`: `truncate_label`, Layout,
  `target_from_args`, `merge_scan_batch`) und Integration-Tests
  (`tests/headless_scan.rs`-Port inkl. CJK-Fixture und Broken-Pipe-Fall) sind grün.
- `.github/workflows/build.yml` baut und releast `04_more_lisp` (Matrix-Eintrag +
  Smoke-Job-Erweiterung); `prompt.txt` ist committet.
- Commits folgen Conventional Commits (s. Abschnitt Commit-Konvention).
- `task.md`, `deps.md`, `walkthrough.md` liegen in `plan/20260919_01_more/`; der
  Walkthrough enthält Transpiler-Lückenvorschläge mit Beispiel-Anwendung,
  umgesetzte Verbesserungen, Learnings und Docker-Programme.

## Context And Current Facts

- `01_more/src/main.rs`, 1440 Zeilen, 8 Abschnitte: (1) `FileNode`
  (`name/is_dir/size_bytes/children/is_sorted/color/target_rect/current_rect/growth_pulse/growth_rate/last_change_time: Instant`,
  Methoden `new/find_mut/update_file_size`); (2) sequentielles Squarify
  (`LayoutWorkspace{areas,row}`, `worst_aspect_ratio/layout_row/squarify_children/collapse_descendants/layout_treemap_sequential`);
  `sum_tree_stats`; (3) Scanner (`DirectoryBatch/ScanEvent`,
  `is_virtual_or_special_fs`, `scan_directory_recursive`, `merge_scan_batch`,
  `merge_scanned_node`); (4) `WatcherManager` (`RecommendedWatcher`-`Option`,
  `Receiver<notify::Result<Event>>`, `HashSet<PathBuf>`, `is_native_recursive/limit_hit/error_msg`,
  `new/register_dir` mit `#[cfg]`-Plattformweiche); (5) Rendering
  (`update_animations/snap_tree/draw_cushion_rect/render_treemap/find_hovered_path`);
  (6) `truncate_label` (Unicode-sicher, CJK-Fix), `get_color_for_filename`,
  `print_line` (Broken-Pipe-tolerant), `format_bytes` (`{:.2}`), `window_conf`;
  (7/8) `target_from_args/resolve_target/has_graphical_display/run_headless`,
  `main` (Headless-Weiche vor `XOpenDisplay`) + `async gui_main`
  (Scanner-Threads, Watcher-Drain, Debounce-Layout, Kamera/Zoom, HUD).
  Deps: `macroquad 0.4`, `notify 6.1` (`default-features = false`, `macos_kqueue`).
- `03_mvp_lisp/` ist die direkte Vorlage: `gen0.lisp` (Loader, 66 Zeilen),
  `lisp/helpers|scan|layout|render|main.lisp` (je 53–83 Zeilen), `lprint`,
  `ext-is`, `report_skip`-Helper, `move`-Closure- und `d /=`-Hatches, `stmt`-Hüllung,
  generierte `format_bytes`-Tests. Walkthrough-Dokumente
  (`plan/20260918_03_transpile_mvp/walkthrough.md`) listen alle Hatches und 4
  Upstream-Vorschläge — vor dem Schreiben lesen.
- Transpiler-Abdeckung (lokal belegt, `transpiler-tests.lisp`): `defstruct0`,
  `make-instance`, `impl`, `defenum`, `attr` (auch `#[cfg(...)]`-tauglich),
  `use`, `case`→`match`, `if-let`/`while-let`/`let-else`, `for`/`while`/`loop`,
  typisierte `lambda`-Closures, `vec!`, `?`, `await`, `defun-async`, `space`
  (generischer Syntax-Hatch, u. a. `pub`), String-Hatch für Lifetimes/Generics/`self`,
  `scope`/`angle`-Turbofish, `dot`-Ketten, `(dot pair 1)`-Tupelzugriff, `aref`,
  unbekannter Head = Funktionsaufruf. Sprachreferenz:
  `/workspace/src/cl-rust-generator/SUPPORTED_FORMS.md`.
- Toolchain-Befund: `sbcl` 2.6.0.debian installiert, `cl-rust-generator` unter
  `~/quicklisp/local-projects/` (nur mit `(ql:register-local-projects)` ladbar);
  `parenmedic` unter `/workspace/src/parenmedic/zig-out/bin/parenmedic`
  (`diagnose`/`fix`); `xvfb-run` fehlt im Container (`which` leer) — Installation
  oder CI-Verlagerung einplanen (s. Risiken); GUI-Smoke-Muster aus
  `plan/20260918_02_review_and_xvfb_test/walkthrough.md` (Erfolg = `timeout`-Exit 124).
- `cargo search` (2026-09-19): `macroquad 0.4.16` = Lock-Stand (kein Update nötig);
  `notify` newest = `9.0.0-rc.5`, `01_more` nutzt `6.1` — kein Upgrade im Transpilat
  (breaking API-Wechsel, s. Entscheidung 9). deepwiki-MCP ist in diesem Container
  ohne Transport nicht verfügbar — Ersatzquellen lokale Registry/`cargo search`/
  docs.rs; `deps.md` hält Abfrageschlüssel vor (Muster `03`-`deps.md`).

## Dateien, die der Agent lesen muss (mit Zweck)

- `01_more/src/main.rs` — einzige Verhaltens-Referenz; abschnittsweise lesen
  (§1–§8, s. Context). Kein Text-Clone — beobachtbares Verhalten zählt.
- `01_more/Cargo.toml`, `01_more/tests/headless_scan.rs` — Dep-Pins und
  Integrationstest-Vorbild (CJK-Fixture, Broken-Pipe-Fall).
- `01_more/plan/` (falls gefüllt) — Vorgänger-Entscheidungen zu `01_more`.
- `03_mvp_lisp/gen0.lisp`, `03_mvp_lisp/lisp/*.lisp` — Generator-Vorbild
  (Modulschnitt, `lprint`, Splices, Hatch-Kommentare).
- `03_mvp_lisp/src/main.rs` (generiert) — Stil-Referenz für erwartetes Emit.
- `plan/20260918_03_transpile_mvp/{plan,task,deps,walkthrough}.md` — Verfahren,
  Commit-Reihenfolge, Hatch-Liste, CI-Muster.
- `plan/20260918_02_review_and_xvfb_test/walkthrough.md` — xvfb-Smoke-Prozedur,
  EPIPE-Lehre, Docker-Pakete.
- `plan/20260918_03_transpile_mvp/gen-cpp-freestanding-example.lisp` —
  freistehendes Transpiler-Beispiel (`sbcl --load ... --quit`-Muster).
- `/workspace/src/cl-rust-generator/SUPPORTED_FORMS.md` — Sprachreferenz für
  jede Emit-Entscheidung (nur belegte Formen verwenden, Rest = Hatch + Vorschlag).
- `/workspace/src/cl-rust-generator/examples/21_mandelbrot/gen00.lisp` —
  Ursprungs-Muster (`lprint`, `write-source`, `*omit-redundant-parens*`).
- `.github/workflows/build.yml` — Matrix- + Smoke-Job-Muster für den `04`-Eintrag.
- `RELEASE_PROCESS.md`, `release-v0.3.0.md` — Release-Konventionen des Repos.

## Constraints And Non-goals

- Code und Deps minimal halten — keine neue Cargo-Dep ohne Spike-Beleg; keine
  Dev-Deps (`tempfile`/`assert_cmd`); Tests via `std`-Mittel wie in `01_more`.
- `00_mvp`, `01_more`, `03_mvp_lisp` bleiben unverändert (Referenzen/Generatoren).
- Kein `notify`-Upgrade (6.1 → 9-rc ist breaking), kein Parallel-Scan, keine neuen
  GUI-Features im Transpilat — 1:1-Abbildung zuerst, Verbesserungen nur begründet.
- Non-goals: CJK-Breitenmodell, `HashMap`-Merge, i18n, Release-Entscheid —
  als Erweiterungen in den Walkthrough.

## Key Decisions

1. **Modularer Generator wie in `03`:** `gen.lisp` = Loader, Emitter-`defun`s in
   `lisp/` (`helpers, data, layout, scan, watcher, render, format, headless, main`),
   je ≤ ~80 Zeilen, Emitter-Fns ≤ 60 Zeilen, Top-Level-Closer einzeln —
   Klammer-Disziplin aus der `03`-Review-Schleife (parenmedic-Probe nach jedem Schritt).
2. **`Cargo.toml` handgeschrieben** (`macroquad 0.4`, `notify 6.1` wie Referenz;
   Mandelbrot-Präzedenz), **nur `src/main.rs` generiert und committet**.
3. **Risiko-Spikes zuerst** (nur `/tmp`): `#[cfg]`, `pub fn` via `space`,
   `Option<RecommendedWatcher>`-/`Receiver<Result<Event>>`-Typstrings,
   Lifetime-Signatur (`find_hovered_path<'a>`), `matches!`-Gebrauch,
   `mouse_wheel().1`-Tupelzugriff, `writeln!`-Broken-Pipe, `move`-Closure
   (bekannt), Tupel-`let` für `(tx, rx)` (bekannt). Jeder Spike: `sbcl`-Exit 0 +
   Fragment per Sichtprüfung/`rustc --crate-type lib` plausibel.
4. **CL-nahe Umformulierungen statt Texttreue:** `worst`/`layout_row`-Extraktion
   (Muster `03`), let-Ketten (`if let ... && let ...`) zu geschachtelten
   `if-let`s, `find_hovered_path`-Lifetime ggf. via String-Hatch oder
   Rückgabe-Umbau mit gleichem Verhalten, `format_bytes`-`{:.2}` beibehalten
   (Referenz!), `size > 0`-Filter aus `03` bewusst **nicht** übernehmen
   (`01_more` zählt leere Dateien — Verhalten der Referenz gilt).
5. **Fehler-Helper faktorisiert:** `report_skip`-Muster aus `03` wiederverwenden;
   Watcher-`eprintln!`-Orte 1:1 übernehmen (Init-/Root-/Limit-Meldungen).
6. **`lprint` + Splices von Anfang an:** Laufzeitstellen Scan-Summary/Layout/Watcher,
   `ext-is`-Farbgruppen und Testdaten via `,@(loop ...)` (Muster `03`).
7. **Generierte Tests im Transpilat:** `truncate_label` (ASCII/CJK/Emoji),
   Layout (Sortierung/Fläche/Overlap), `target_from_args`, `merge_scan_batch`,
   Scanner-Fixture — aus `01_more`-`mod tests` abgeleitet, via Splices wo repetitiv.
8. **CI additiv:** neuer Matrix-Eintrag (`dir: 04_more_lisp`, `bin: treemap_more_lisp`)
   + Smoke-Job-Erweiterung (fmt/clippy/test, Headless- + GUI-Smoke); YAML per
   `yaml.safe_load` validiert.
9. **Kein Dep-Wechsel ohne Beleg:** `macroquad`/`notify` bleiben; Ersatz-Kandidaten
   nur per Spike (Build + Smoke) — Ergebnis in `deps.md`/Walkthrough.
10. **Rollback:** `04_more_lisp/` ist neu — Löschen stellt den Vorzustand her;
    jeder Commit einzeln revertierbar.

## Recommended Approach

1. Spikes (Entscheidung 3) klären die offenen Transpiler-Stellen; Befunde bestimmen
   Hatch-Bedarf und `find_hovered_path`-Form.
2. Gerüst (`Cargo.toml`, `gen.lisp`, `lisp/helpers.lisp`) + erster Generierungslauf
   (leeres `do0` → `cargo build`-Gerüst grün).
3. Module seriell aufbauen (Reihenfolge s. `task.md`): data → layout → scan →
   watcher → render → format → headless → main; nach jedem Modul generieren,
   parenmedic-Probe, `cargo build`/`fmt`/`clippy`.
4. Tests portieren (generierte Unit-Tests + `tests/headless_scan.rs`-Port),
   Äquivalenz belegen (Diff informativ, Headless-Fixture, `lprint`-stderr, xvfb-Smoke).
5. CI erweitern, Release-Pfad lokal belegen (`cargo build --release`).
6. Dokumente/Commits in `task.md`-Reihenfolge; `walkthrough.md` zuletzt.

## Validation Plan

- `sbcl --load 04_more_lisp/gen.lisp --quit` → Exit 0, `src/main.rs` geschrieben.
- `cd 04_more_lisp && cargo build`, `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings` → grün.
- `cargo test` (generierte Unit-Tests + portierte Integration-Tests) → grün.
- `sh /workspace/src/cl-rust-generator/run-tests.sh` → 173/173 (keine Regression).
- `diff 01_more/src/main.rs 04_more_lisp/src/main.rs` gelesen; nur begründete
  Differenzen (CL-Umformulierung, Helper, Float-Druck, Klammer-Hatches).
- Headless-Fixture (CJK + Broken-Pipe) wie `01_more` → Exit 0, Summary + `180`-Total.
- `lprint`-Zeilen auf stderr im Smoke sichtbar.
- `xvfb-run -a timeout 20 ./target/debug/<bin> <dir>` → Exit 124, kein Panic
  (Referenz parallel; Muster `02`-Walkthrough). Höchstrisiko: GUI-Laufzeit des
  Transpilats (Watcher-/Thread-Semantik).
- `python3 -c "import yaml; yaml.safe_load(...build.yml...)"` grün +
  `cargo build --release` grün.
- `git status --short`: nur beabsichtigte Pfade, nie `target/`.

## Risks / Rollback

- `find_hovered_path`-Lifetime ohne Support → String-Hatch oder Rückgabe-Umbau
  (Index statt Referenz), verhaltensgleich.
- `#[cfg]`-Per-OS-Arme im Watcher/HUD → `attr`-Spike; Fallback String-Hatch.
- `matches!(event.kind, EventKind::Access(_))` → Spike; Fallback `case`-Match
  auf `event.kind` mit Pattern-Arm.
- let-Ketten → geschachtelte `if-let` (sicherer Pfad, kein Hatch nötig).
- `xvfb-run` fehlt im Container → `apt-get install xvfb` (Pakete s. Walkthrough
  `02`) oder Smoke in CI; kein Blocker für Generierung/Tests.
- `notify 6.1` unter Wayland-losen CI-Containern: Watcher-Init-Fehler werden
  gemeldet, nicht panisch (Referenzverhalten) — Smoke prüft Exit, nicht Watches.
- Rollback: neues Verzeichnis + additive CI — Löschen/Revert stellt Vorzustand her.

## Open Questions

1. Binärname `treemap_more_lisp` (Vorschlag) OK? Default: ja.
2. `find_hovered_path`-Rückgabetyp bei fehlendem Lifetime-Support: String-Hatch
   oder Index-Umbau? Default: Spike entscheidet.
3. `notify`-Pin 6.1 trotz 9.0-rc-Newest akzeptiert (breaking)? Default: ja.

## Commit-Konvention (für den ausführenden Agenten)

Conventional Commits, imperativ, Format `<type>(<scope>): <Kurzbeschreibung>`,
Typen `feat`/`fix`/`test`/`docs`/`chore`/`ci`. Jede Message mit Body aus
Was/Warum/Tests (je 1–3 Zeilen). Scopes: `04_more_lisp`, `plan`, `ci`.
Reihenfolge: `docs(plan)` (Plan/Tasks/Deps + `prompt.txt`) →
`feat(04_more_lisp)` (Generator + Generat, ggf. gestapelt pro Modul) →
`test(04_more_lisp)` (portierte Tests, falls separat) → `ci` (Matrix + Smoke) →
`docs(plan)` (Walkthrough). Nur eigene Pfade stagen (`git add <pfade>`), nie
`git add -A`, nie `target/`. Beispiel:

```text
feat(04_more_lisp): generate treemap analyzer from gen.lisp via transpiler

Port 01_more (scanner, watcher, layout, GUI) to modular gen.lisp emitters.
Generated src/main.rs committed for reviewability.

Tests: cargo build, fmt --check, clippy -D warnings, cargo test green.
```

## Requirements-Abgleich („Habe ich alle Requirements?“)

Explizit gefordert und abgedeckt: `gen.lisp` nach `gen00.lisp`-Muster,
`sbcl`-Generierung, `SUPPORTED_FORMS.md`-Referenz, freistehendes Beispiel als
Vorlage, Klammer-Disziplin mit parenmedic + Backups, ≤60-Zeilen-Funktionen mit
einzelnen Top-Level-Closer, xvfb-Tests (Muster `02`), kleine Code/Deps-Basis mit
Tools, Architektur-Sicht, deepwiki-Doku + `deps.md` mit Orgs (deepwiki ohne
Transport → Schlüssel vorbereitet), neueste Versionen + Usage-Examples
(s. `deps.md`), Feature-Vorschlag (s. unten), Plan mit Dateiliste (s. oben) +
Commit-Konvention (s. oben), Unit-/Integration-Tests + Ausführung, `task.md`,
`walkthrough.md` + Docker-Programme.

Vorgeschlagene Ergänzungen (im Plan eingebaut): Risiko-Spikes vor dem Schreiben;
Source-of-Truth-Regel (nur `main.rs` generiert, `Cargo.toml` handgeschrieben);
Diff-gegen-Referenz als informatives (nicht normatives) Kriterium; Hatch-Budget
mit Upstream-Pfad in den Walkthrough; `01_more`-Verhalten (`{:.2}`, leere Dateien
zählen) geht vor `03`-Gewohnheit (`{:.1}`, `size > 0`-Filter).

## Feature-Vorschläge (was man am besten umsetzen könnte)

1. Transpilierung wie beauftragt (Prio 1, dieser Plan).
2. `size > 0`-Frage sichtbar machen: Zähler für übersprungene 0-Byte-Dateien in
   die Headless-Summary (klein, testbar).
3. Inotify-Limit-Hinweis (`sysctl ...max_user_watches`) als generierte Konstante
   statt String-Duplikat.
4. `format_bytes`-Genauigkeit (`{:.2}`) per generiertem Unit-Test festnageln
   (Regression gegen `03`-`{:.1}`-Gewohnheit).
5. Später: `notify 9`-Evaluierung als eigener Spike (breaking, separater Change),
   paralleler Scan, CJK-Breitenmodell.
