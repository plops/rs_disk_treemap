## Goal

`00_mvp` (286 Zeilen, Referenz-MVP, eingefroren) per `cl-rust-generator`-Transpiler aus einer Lisp-Quelle (`gen.lisp`, Muster: `examples/21_mandelbrot/gen00.lisp`) nach `03_mvp_lisp/` überführen. `gen.lisp` ist danach die Source of Truth, `src/main.rs` wird per `sbcl` generiert. Das Transpilat erhält `lprint`-Logging (Semantik wie im C++-Beispiel: Ausdrücke werden automatisch stringifiziert; repetitive Generierung via Splices/`loop` und Lisp-`defun`s faktorisiert). Verhalten: äquivalent zum Referenz-MVP, aber ohne dessen stille `Err`-Ignoranz — Fehler werden über eine faktorisierte Lösung gemeldet (kein Text-Clone — die Input-Sprache ist CL-ähnlich, Verbesserungen und ggf. bessere Dependencies sind explizit erwünscht und werden gleich umgesetzt). Lisp-Input wie generierter Rust-Code sollen schön kurz bleiben. Ergebnis: laufendes `03_mvp_lisp/`-Projekt mit GitHub-Release-Build, plus Plan-Dokumente (`plan.md`, `task.md`, `deps.md`, `walkthrough.md` in `plan/20260918_03_transpile_mvp/`).

## Success Criteria

- `03_mvp_lisp/gen.lisp` existiert, folgt dem `gen00.lisp`-Muster (`ql:quickload`, `in-package`, `write-source` mit `` `(do0 ...) ``) und generiert per `sbcl`-Lauf ohne Fehler `03_mvp_lisp/src/main.rs`.
- Generierter Code baut mit `cargo build`, ist `cargo fmt --check`-sauber und `cargo clippy`-warnungsfrei (ggf. via `*rustfmt-program*`-Kette belegt).
- Verhaltens-Äquivalenz zu `00_mvp` ist belegt (Diff-Analyse plus GUI-Smoke unter `xvfb-run`; Details s. Validation Plan). Abweichungen sind dokumentiert.
- Unit-/Integration-Tests für die Transpilierung sind eingeführt und grün (Transpiler-Suite-Regression plus Generierungs-Smoke).
- Commits folgen Conventional Commits (`<type>(03_mvp_lisp|plan): <Imperativ>` + Body mit Was/Warum/Tests; Typen `feat`/`test`/`docs`/`chore`), nur eigene Pfade, nie `target/`.
- `task.md`, `deps.md`, `walkthrough.md` liegen in `plan/20260918_03_transpile_mvp/`.
- `gen.lisp` definiert ein `lprint` (C++-Beispiel-Semantik: übergebene Ausdrücke erscheinen automatisch stringifiziert in der Ausgabe, ein Aufruf pro Logzeile) und nutzt es an mindestens zwei Stellen des Transpilats (z. B. Scan-Ergebnis, Layout-Kennzahlen); repetitive Generierung ist via `,@(loop ...)`-Splices und Lisp-`defun`s faktorisiert.
- `.github/workflows/build.yml` baut und releast `03_mvp_lisp` mit (Matrix-Eintrag + Smoke-Job-Erweiterung); `prompt.txt` und `gen-cpp-freestanding-example.lisp` sind committet.
- `walkthrough.md` enthält für jede während der Umsetzung gefundene Transpiler-Lücke einen Vorschlag mit Beispiel-Anwendung.
- Kein stilles Verschlucken von `Err`-Fällen: alle drei Fehlerstellen aus `scan_tree` (`read_dir`, `file_type`, `metadata`) melden sich über genau eine generierte Rust-Helper-`fn` (ein `eprintln!`-Ort, von einem Lisp-`defun` emittiert); das Transpilat bleibt dabei kurz (kein wiederholter Fehler-Block).

## Context And Current Facts

- `00_mvp/src/main.rs` (286 Zeilen, Kommentar „eingefroren“ am Dateikopf): `Node`-Struct (`path/size/is_dir/children/rect/color`), `scan_tree` (rekursiv, Sync-`fs::read_dir`, Symlink-Skip, `/proc|/sys|/dev`-Filter, `size > 0`-Filter), `squarify` mit zwei Closures (`worst`, `layout_row`), `render_tree`, `color_for_path` (Extension-`match`), `format_bytes`, `main` mit `#[macroquad::main]`, Bg-Thread + `channel()`. Einzige Dep: `macroquad 0.4`.
- Drei stille `Err`-Stellen in `scan_tree` (alle ohne Meldung): `fs::read_dir(path)` → `if let Ok` (leerer Knoten bei Fehler), `entry.file_type()` → `continue` bei `Err`, `entry.metadata()` → `unwrap_or(0)`. Diese werden im Transpilat durch eine einzige faktorisierte Meldelösung ersetzt (s. Entscheidung 5).
- `01_more` (1440 Zeilen) ist **nicht** Transpilierungsziel — zu groß für einen Erst-Transpiler-Schritt; `03` bildet bewusst nur `00_mvp` ab.
- Transpiler-Muster `examples/21_mandelbrot/gen00.lisp`: `(ql:quickload "cl-rust-generator")`, `(in-package :cl-rust-generator)`, `*source-dir*`/`*code-file*`, `lprint`-Helper, `(let ((*omit-redundant-parens* t)) (write-source ...))`. `Cargo.toml` des Beispiels (`mandelbrot/Cargo.toml`: `image`, `num`) ist **handgeschrieben** — Präzedenz: nur `main.rs` wird generiert.
- Transpiler-Abdeckung (lokal verifiziert in `transpiler-tests.lisp`, Testnamen via `grep :name`): `defstruct0`, `make-instance`, `impl`, `defun` (typisiert, `&`/`&mut`-Parameter, String-Escape-Hatch für `&mut self`/Lifetimes), `attr` (`#[...]`-Zeilen, z. B. `derive`), `use`, `case`→`match` (`t` = `_`), `for`/`dotimes`/`while`/`loop`, `lambda`-Closures (typisiert), `vec!`, `?`, `await`, `do0`/`progn`/`block`, unbekannter Head = Funktionsaufruf (`function-call`-Test). `println!` im Paren-Stil ist belegt (`defun-untyped`-Test); Tupel-Destrukturierung im `let` nutzt das Beispiel-Gen (`(tuple width height)` in `gen00.lisp`, ca. Zeile 99).
- Toolchain-Befund dieser Sitzung: `sbcl` (2.6.0.debian) ist installiert, `cl-rust-generator` liegt unter `~/quicklisp/local-projects/`. Bloßes `(ql:quickload "cl-rust-generator")` schlägt fehl (`SYSTEM-NOT-FOUND`); mit `(ql:register-local-projects)` davor greift die Registrierung (Muster s. `run-tests.sh`). Test-Suite läuft via `run-tests.sh` (`run-transpiler-tests`, Exit ≠ 0 bei Fail); `generate-documentation` erzeugt `SUPPORTED_FORMS.md`.
- Vorbild-Dokumente: `plan/20260918_02_review_and_xvfb_test/{plan,task,deps,walkthrough}.md` (Stil, Commit-Konvention, xvfb-Smoke mit `timeout`-Exit 124 = Event-Loop lief, EPIPE-Lehre). `deps.md` dort belegt: **kein** deepwiki-MCP-Transport in diesem Container — Ersatzquellen lokale Registry/`cargo search`/docs.rs; deepwiki-Schlüssel trotzdem notieren.
- Verfahrene Lesart des Auftrags: Zielverzeichnis heißt im Prompt `03_mpv_lisp` (vermutlich Tippfehler). Vorgeschlagen: `03_mvp_lisp` (s. Offene Fragen).
- `lprint`-Vorbild `plan/20260918_03_transpile_mvp/gen-cpp-freestanding-example.lisp`: Lisp-`defun lprint (&key msg vars)` baut aus `vars` (Ausdruck oder `(:as "label" expr ["spec"])`) genau einen `LS_LOG_INFO`-Aufruf; Labels werden via `(emit-c :code v)` aus den Ausdrücken gewonnen. Rust-Analogon (Muster `gen00.lisp`-`lprint`): ein `eprintln!` mit `{:?}`-Platzhaltern, Labels via `(emit-rs :code e)`.
- Release-CI (lokal gelesen, `.github/workflows/build.yml`): `build`-Job mit Matrix (`00_mvp`, `01_more` × `ubuntu`/`windows`), Release-Upload via `softprops/action-gh-release` bei `v*`-Tags; `smoke`-Job (Linux + `xvfb`): `fmt`/`clippy`/`test` + Headless- und GUI-Smoke für `01_more`. Ein `03`-Eintrag folgt demselben Muster (neuer `dir`/`name`/`bin`-Eintrag, `swatinem/rust-cache`-Workspace ergänzen).
- Semantik-Vorgabe: Die Transpiler-Input-Sprache ist an Common Lisp angelehnt — der generierte Code muss **nicht** textuell wie `00_mvp` aussehen. Äquivalenzkriterium ist beobachtbares Verhalten (Scan-Ergebnis, Layout, GUI-Liveness), der Diff gegen `00_mvp` ist informativ, nicht normativ.

## Constraints And Non-goals

- Vorgabe: Code und Abhängigkeiten minimal halten — **keine** neue Cargo-Dep ohne Beleg; keine `tempfile`/`assert_cmd`-Dev-Deps (Smoke via `std::process::Command`, Muster `01_more/tests/headless_scan.rs`).
- `00_mvp` bleibt unverändert (eingefroren); `01_more` ist nicht im Scope.
- Kein `notify`-Upgrade, keine Headless-Optionen, kein Parallel-Scan im Transpilat — 1:1-Abbildung zuerst.
- Non-goals: CJK-Breiten, `HashMap`-Merge, Release-Workflow-Entscheid (beide Binaries?) — bleiben bei `02` bzw. werden als Erweiterung notiert.

## Key Decisions

1. **Nur `00_mvp` transpilieren, `gen.lisp` wird Source of Truth.** Alternative „`01_more` gleich mit“ verworfen: 1440 Zeilen mit Watcher/Animation/Kamera vervielfachen das Transpiler-Risiko; inkrementell ist billiger.
2. **`Cargo.toml` handgeschrieben (`macroquad 0.4`), nur `main.rs` generiert.** Folgt dem Mandelbrot-Präzedenzfall; kein Generator für Manifeste nötig. Neueste-Version-Check per `cargo search macroquad` bei Umsetzung (bei Neueinführung sonst neueste Version nehmen).
3. **Verzeichnis `03_mvp_lisp/`** (Korrektur des `03_mpv_lisp`-Tippfehlers; bei Ablehnung umbenennen, s. Offene Fragen). Inhalt: `gen.lisp`, `Cargo.toml`, generiertes `src/main.rs`.
4. **String-Escape-Hatch nur als dokumentierte Ausnahme.** Der Transpiler kennt String-Formen als Ausweg (Tests `string-escape-hatch`, `defun-string-parameter`). Jede Nutzung im `gen.lisp` wird kommentiert und im Walkthrough als Upstream-Kandidat (`transpiler-tests.lisp`-Fall) gelistet.
5. **Fehler melden statt schlucken — faktorisiert und kurz.** Die drei `Err`-Stellen aus `scan_tree` werden nicht 1:1 übernommen: Ein Lisp-`defun` (z. B. `report-skip`) emittiert genau eine kleine Rust-Helper-`fn` mit einem einzigen `eprintln!`-Ort (Kontext + Pfad + Fehler, `{:?}`); alle drei Stellen rufen nur diese auf (Einzeiler pro Stelle, kein wiederholter Block). Idiomatische Formen dafür sind Transpiler-Spike-Kandidaten (`case` auf `Result`, `let-else`, `unwrap_or_else` mit Melde-Closure); was fehlt, wird als Walkthrough-Vorschlag mit Beispiel notiert. Unverändert bleiben: `size > 0`-Filter (0-Byte-Dateien unsichtbar — dokumentierte Designwahl, Fix-Vorschlag in den Walkthrough), Box-Rekursion.
6. **Generiertes `src/main.rs` wird committet** (reviewbar, reproduzierbar; Generator-Lauf in `task.md` belegt den Weg dorthin). Alternative „nur `gen.lisp` versionieren“ verworfen: Reviewer ohne `sbcl` könnten sonst nichts prüfen.
7. **Risiko-Spikes zuerst** (`move`-Closure für `thread::spawn`, Glob-`use macroquad::prelude::*`, `format!` mit benannten Args, Tupel-`let` für `(tx, rx)`): je ein minimaler Probe-`write-source`-Lauf nach `/tmp`, bevor `gen.lisp` wächst. Fallback pro Lücke: String-Hatch oder kleinste semantikerhaltende Umformulierung (als bewusste Verbesserung zu kennzeichnen — Texttreue ist kein Ziel). Jede Lücke wird mit Beispiel-Anwendung für `walkthrough.md` notiert (Upstream-Vorschlag).
8. **`lprint` nach C++-Semantik, mit Splices faktorisiert.** `gen.lisp` erhält `(defun lprint (&key (msg "") (vars nil)))`, das aus jedem Ausdruck via `(emit-rs :code e)` Label + `{:?}`-Wert baut und genau ein `eprintln!` emittiert (Vorbild: `gen00.lisp`-`lprint`); `(:as "label" expr)`-Form für eigene Labels. Repetitive Stellen (z. B. `color_for_path`-Arme, Logzeilen, Testdaten) werden mit `,@(loop for ... collect ...)` generiert, nicht ausgeschrieben.
9. **Verbesserungen und bessere Dependencies sofort umsetzen.** Findet die Umsetzung eine Vereinfachung (z. B. andere/neue Library statt `macroquad`-Handaufwand, idiomatischere CL-Formen), wird sie im Transpilat umgesetzt — mit `cargo search`-Newest-Check, Usage-Example im Plan-`deps.md`, GitHub-Org-Eintrag und Begründung im Walkthrough. Kein Upgrade ohne Beleg harmloser Kompatibilität (Muster: `02`-`notify`-Zurückhaltung).
10. **Release-CI für `03` und Beispiel-Dateien committen.** `build.yml`: neuer Matrix-Eintrag (`dir: 03_mvp_lisp`, `bin`-Name) + `smoke`-Job um `03`-Fmt/Clippy/Build und GUI-Smoke erweitern. `prompt.txt` und `gen-cpp-freestanding-example.lisp` werden in einem `docs(plan)`-Commit mit aufgenommen (eigene Pfade, nie `git add -A`).

## Recommended Approach

1. Spikes (s. Entscheidung 7) klären die vier offenen Transpiler-Stellen; Ergebnisse bestimmen, wie viele String-Hatches `gen.lisp` braucht.
2. `gen.lisp` abschnittsweise aufbauen, spiegelbildlich zu `00_mvp` (§1–§4: Traversal, Layout, Rendering/Utils, Main-Loop), mit `*omit-redundant-parens*`-Bindung wie in `gen00.lisp`; `lprint`-`defun` plus `,@(loop ...)`-Splices von Anfang an mitführen und an ≥2 Laufzeitstellen (Scan-Summary, Layout-/Hover-Info) einsetzen.
3. Generieren → `cargo build` → `cargo fmt`/`clippy` → Diff gegen `00_mvp/src/main.rs` lesen (informativ: CL-nahe Umformulierungen und umgesetzte Verbesserungen sind erwartet; nur unbeabsichtigte Verhaltensdifferenzen werden beseitigt).
4. Äquivalenz belegen: Transpiler-Suite grün (`run-tests.sh`), Generierungs-Smoke, `lprint`-Ausgaben auf stderr prüfen, beide GUIs unter `xvfb-run` (Erfolg = `timeout`-Exit 124, Muster aus `02`-Walkthrough).
5. CI erweitern (`build.yml`: `03`-Matrix-Eintrag + Smoke), YAML per `yaml.safe_load` validieren, Release-Pfad lokal per `cargo build --release` belegen.
6. Dokumente und Commits in der Reihenfolge aus `task.md` (nach Approval zu schreiben); `walkthrough.md` zuletzt mit Spikes-Ergebnissen, Hatch-Liste, Transpiler-Lückenvorschlägen mit Beispielen, umgesetzten Verbesserungen, Learnings, Docker-Programmen. `prompt.txt` + C++-Beispiel werden committet.

## Work Plan

- Phase A — Spikes & Baseline: `00_mvp`-Build grün sichern; vier Probe-Läufe (`move`-Closure, Glob-`use`, `format!`, Tupel-`let`) nach `/tmp`; Befunde entscheiden Hatch-Bedarf.
- Phase B — `03_mvp_lisp/`-Gerüst: `Cargo.toml` (hand, `macroquad 0.4`, newest-check), `gen.lisp`-Skelett (`quickload` + `register-local-projects`, `in-package`, `write-source`, Abschnitte §1–§4), erster Generierungslauf.
- Phase C — Vollständige Abbildung: `scan_tree` (inkl. faktorisierter `Err`-Meldung über eine Helper-`fn`), `squarify` (+ Closures), `render_tree`/`color_for_path`/`format_bytes`, `main`-Loop; `lprint`-Einsatzstellen + `,@(loop ...)`-Faktorisierung; Kürze prüfen (kein duplizierter Fehler-Block, keine ausgeschriebenen Wiederholungen); Schleife Generieren→Build→Fmt→Clippy→Diff bis grün.
- Phase D — Tests, Äquivalenz & CI: Transpiler-Regression (`run-tests.sh`), Generierungs-Smoke-Skript, `cargo build`, Diff-Review (informativ), `lprint`-stderr-Prüfung, `xvfb`-GUI-Smoke beider Binaries; `build.yml` um `03`-Matrix + Smoke erweitern, YAML validieren, `cargo build --release` lokal belegen.
- Phase E — Dokumente & Commits: `task.md`, `deps.md` (Dep-Tabelle mit GitHub-Org + deepwiki-Schlüssel, newest-check, Usage-Example bei neuer/gewechselter Dep), `prompt.txt` + C++-Beispiel committen, Commits in Konvention, `walkthrough.md` (Spikes, Hatches, Transpiler-Lückenvorschläge mit Beispielen, umgesetzte Verbesserungen, Learnings, Erweiterungen, Docker-Programme).

## Validation Plan

- `sbcl --load 03_mvp_lisp/gen.lisp` (mit `register-local-projects`): Exit 0, `src/main.rs` geschrieben/geändert-Meldung.
- `cd 03_mvp_lisp && cargo build`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`: jeweils grün.
- `sh /workspace/src/cl-rust-generator/run-tests.sh`: grün (keine Transpiler-Regression durch neue Formen).
- `diff 00_mvp/src/main.rs 03_mvp_lisp/src/main.rs`: gelesen; CL-nahe Umformulierungen/Verbesserungen erwartet, nur unbeabsichtigte Verhaltensdifferenzen sind Blocker.
- Generiertes `main.rs` enthält `lprint`-expandierte `eprintln!`-Zeilen; deren stderr-Ausgabe ist im `xvfb`-Lauf sichtbar.
- Fehler-Fixture (nicht existenter Pfad + unlesbares Verzeichnis): Transpilat meldet jede Stelle genau einmal auf stderr (Helper-`fn`-Text), kein Panic, Exit-Verhalten wie Referenz (leerer Baum statt Absturz).
- `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/build.yml'))"` grün + `cargo build --release` in `03_mvp_lisp/` grün (Release-Pfad lokal belegt; GH-Release selbst läuft erst bei `v*`-Tag).
- `xvfb-run -a timeout 20 ./00_mvp/target/debug/treemap_mvp <dir>` und `xvfb-run -a timeout 20 ./03_mvp_lisp/target/debug/<bin> <dir>`: beide Exit 124, kein Panic auf stderr (beim Transpilat sind zusätzlich die `lprint`-Zeilen sichtbar; Muster `02`-Walkthrough). Höchstrisiko-Schritt: GUI-Smoke des Transpilats (fängt Laufzeit-Divergenzen wie falsche `move`-Semantik).
- `git status --short`: nur beabsichtigte Pfade, keine `target/`-Verzeichnisse.

## Risks / Rollback

- `move`-Closure ohne Transpiler-Support → Fallback String-Hatch oder `thread::spawn`-Umformulierung; notfalls Bg-Thread synchron (als dokumentierte Abweichung).
- Glob-`use` (`prelude::*`) nicht emittierbar → explizite Importliste oder String-Hatch.
- `format!`-Sonderformen (benannte Args) → Paren-Stil verifizieren, sonst Hatch.
- `#[macroquad::main("...")]`-Attr mit String-Arg → `attr`-Form verifizieren (Beleg bisher nur `derive`), sonst Hatch.
- Rollback: `03_mvp_lisp/` ist neues Verzeichnis, `00_mvp` unangetastet — Löschen des Verzeichnisses stellt den Vorzustand her; jeder Commit ist einzeln revertierbar. CI-Änderung ist additiv (eigener Matrix-Eintrag) und per Revert isolierbar.
- Dep-Wechsel-Risiko: Falls eine Ersatz-Dep evaluiert wird, erst Spike (`cargo build` + Smoke) vor Übernahme; sonst bleibt `macroquad 0.4` (bewährter `02`-Stand).

## Open Questions

1. Verzeichnisname `03_mvp_lisp` statt `03_mpv_lisp` (Tippfehler-Korrektur)? Default: ja.
2. `Err`-Stellen melden statt schlucken (entschieden 18.09.: ja, faktorisiert über eine Helper-`fn`); 0-Byte-Filter und Box-Rekursion unverändert übernehmen? Default: ja, Fixes nur als Walkthrough-Vorschlag.
3. Generiertes `src/main.rs` committen? Default: ja (Reviewbarkeit).
4. deepwiki-MCP (`plops/rs_disk_treemap`) steht hier ohne Transport nicht zur Verfügung — als Beleglücke akzeptiert, `deps.md` hält Abfrageschlüssel vor? Default: ja.

## Requirements-Abgleich (Auftrag § „habe ich alle Requirements?“)

Explizit gefordert und abgedeckt: `gen.lisp` nach `gen00.lisp`-Muster, `sbcl`-Generierung, `SUPPORTED_FORMS.md` als Sprachreferenz, freistehendes Beispiel als Vorlage, `xvfb`-Tests (Muster `02`-Walkthrough), kleine Code/Deps-Basis mit Tools, Architektur-Sicht, deepwiki-Doku + `deps.md` mit Orgs, neueste Versionen + Usage-Examples, Feature-Vorschlag, Plan mit Dateiliste + Commit-Konvention, Unit-/Integration-Tests + Ausführung, `task.md`, `walkthrough.md` + Docker-Programme.

Vorgeschlagene Ergänzungen (im Plan bereits eingebaut): Transpiler-Risiko-Spikes vor dem Schreiben (Entscheidung 7); Source-of-Truth-Regel (`gen.lisp` vs. generiertes `main.rs`, Entscheidung 6); Diff-gegen-Referenz als Äquivalenzkriterium (Validation); Hatch-Budget mit Upstream-Pfad (Entscheidung 4); faktorisierte `Err`-Meldung statt stiller Fixes, Rest 1:1 (Entscheidung 5); Rollback-Eigenschaft (neues Verzeichnis).

Neu aus deiner Nachricht vom 18.09. (eingearbeitet, Entscheidungen 8–10): `lprint`-Logging mit C++-Semantik (Auto-Stringifizierung, ein Aufruf pro Zeile) plus Splice-/`defun`-Faktorisierung; GitHub-Action baut und releast `03` mit; `prompt.txt` + C++-Beispiel werden committet; Transpiler-Lücken werden im Walkthrough mit Beispiel-Anwendung vorgeschlagen; CL-Ähnlichkeit geht vor Texttreue (generierter Code darf besser aussehen als `00_mvp`); gefundene Verbesserungen — inkl. neuer/anderer Libraries — werden gleich umgesetzt (mit Newest-Check, Usage-Example, `deps.md`-Eintrag).

Korrektur zu Entscheidung 5 (dein Einwand 18.09.): stille `Err`-Ignoranz wird **nicht** übernommen — alle drei Fehlerstellen melden sich über genau eine generierte Helper-`fn` (ein Lisp-`defun`, Einzeiler pro Aufrufstelle); Lisp-Input und Rust-Code bleiben kurz.
