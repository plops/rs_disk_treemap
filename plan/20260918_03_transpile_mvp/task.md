# task.md — Serielle Implementierungs- und Testschritte (03-Transpilierung)

Jeder Schritt endet mit Validierung; erst bei Grün weiter. Scope-Entscheidungen
(2026-09-18, approved): nur `00_mvp` transpilieren nach `03_mvp_lisp/`,
`gen.lisp` ist Source of Truth, `lprint`-Logging mit C++-Semantik,
faktorisierte `Err`-Meldung (kein stilles Schlucken), CL-Nähe vor Texttreue,
Verbesserungen sofort umsetzen, `03` in Release-CI, `prompt.txt` + C++-Beispiel
committen, Transpiler-Lücken im Walkthrough mit Beispiel vorschlagen.
Details s. `plan.md`.

## Schritt 0 — Baseline sichern (keine Codeänderung)

1. `cd 00_mvp && cargo build` → ✅ grün (Stand 2026-09-18).
2. `sbcl --eval '(ql:register-local-projects)' --eval '(ql:quickload "cl-rust-generator")'`
   → ✅ lädt (ohne `register-local-projects` schlägt es fehl — bekannter Befund).
3. `sh /workspace/src/cl-rust-generator/run-tests.sh` → ✅ grün (Ausgangsbasis).
4. `git status --short` prüfen; `target/`-Verzeichnisse nie stagen.

## Schritt 1 — Risiko-Spikes (nur `/tmp`, nichts committen)

Je ein minimaler `write-source`-Lauf nach `/tmp`, Ergebnis prüfen:

1. `move`-Closure für `thread::spawn` (Muster: `(thread--spawn (move (lambda ...)))`
   o. ä.) → lesbares `thread::spawn(move || ...)`?
2. Glob-`use` (`macroquad::prelude::*`) → Alternative: explizite Liste oder String-Hatch.
3. `format!` mit benannten Args + `eprintln!`-Paren-Stil → `lprint`-Bauform festlegen.
4. Tupel-`let` für `(tx, rx)` (`channel()`-Destrukturierung).
5. `Err`-Formen: `case` auf `Result`, `let-else`, `unwrap_or_else` mit Melde-Closure.
6. ✅ Jeder Spike: `sbcl`-Exit 0 + generiertes Fragment per `rustc --crate-type lib`
   oder Sichtprüfung plausibel. Befunde + Lücken mit Beispiel notieren
   (Vorlage für Walkthrough-Vorschläge).

## Schritt 2 — Gerüst `03_mvp_lisp/`

1. `03_mvp_lisp/Cargo.toml` per Hand (Muster `mandelbrot/Cargo.toml`):
   `macroquad 0.4` nach `cargo search macroquad`-Newest-Check; abweichende
   Dep nur mit Spike-Beleg (s. Plan-Entscheidung 9).
2. `gen.lisp`-Skelett: `quickload` + `register-local-projects`, `in-package`,
   `*source-dir*`/`*code-file*` (Ziel `03_mvp_lisp/src/main.rs`), `lprint`-`defun`,
   `report-skip`-`defun` (Err-Helper), `(let ((*omit-redundant-parens* t))
   (write-source ...))` mit `do0`-Rumpf für §1–§4.
3. Erster Generierungslauf `sbcl --load 03_mvp_lisp/gen.lisp` → ✅ Exit 0,
   `src/main.rs` geschrieben.
4. ✅ `cd 03_mvp_lisp && cargo build` grün (ggf. noch lückenhaft — OK in Schritt 2).

## Schritt 3 — Vollständige Abbildung

1. `scan_tree` (mit Helper-`fn`-Aufrufen an den 3 `Err`-Stellen),
   `squarify` (+ Closures), `render_tree`/`color_for_path`/`format_bytes`,
   `main`-Loop; `lprint` an ≥2 Laufzeitstellen; `,@(loop ...)`-Splices für
   repetitive Stellen (Farb-Arme, Unit-Testdaten); generierte `#[cfg(test)]`-
   Unit-Tests für `format_bytes` (Muster `gen00.lisp`-`test_parse_pair`).
2. Schleife Generieren → `cargo build` → `cargo fmt` → `cargo clippy
   --all-targets -- -D warnings` → Diff gegen `00_mvp` lesen (informativ).
3. ✅ Build + `fmt --check` + `clippy` warnungsfrei; ✅ Kürze: genau eine
   Helper-`fn`, keine duplizierten Fehler-Blöcke, keine ausgeschriebenen
   Wiederholungen.

## Schritt 4 — Tests & Äquivalenz

1. `sh /workspace/src/cl-rust-generator/run-tests.sh` → ✅ grün (keine Regression).
2. `cd 03_mvp_lisp && cargo test` → ✅ grün (generierte Unit-Tests).
3. Fehler-Fixture: nicht existenter Pfad + unlesbares Verzeichnis unter `xvfb-run`
   → ✅ jede Stelle genau einmal auf stderr (Helper-Text), kein Panic,
   leerer Baum statt Absturz.
4. `lprint`-stderr-Prüfung im `xvfb`-Lauf → ✅ Scan-Summary sichtbar.
5. GUI-Smoke beider Binaries (`xvfb-run -a timeout 20 ...`) → ✅ beide Exit 124,
   kein Panic auf stderr.

## Schritt 5 — CI: `03` in Release-Build

1. `.github/workflows/build.yml`: Matrix-Eintrag (`dir: 03_mvp_lisp`, …) +
   `smoke`-Job um `03` (`fmt`/`clippy`/`build`, GUI-Smoke) erweitern.
2. ✅ `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/build.yml'))"`.
3. ✅ `cd 03_mvp_lisp && cargo build --release` grün (GH-Release selbst läuft
   erst bei `v*`-Tag).

## Schritt 6 — Dokumente + Commits (Conventional Commits, nur eigene Pfade!)

1. `task.md` (diese Datei), `deps.md` finalisieren (Newest-Check, Usage-Example
   bei neuer/gewechselter Dep, GitHub-Org + deepwiki-Schlüssel).
2. Commits in Reihenfolge (Format s. `plan.md`):
   1. `docs(plan): add 03_transpile plan, tasks and deps` (+ `prompt.txt`,
      C++-Beispiel),
   2. `feat(03_mvp_lisp): generate treemap MVP from gen.lisp via transpiler`
      (`gen.lisp`, `Cargo.toml`, generiertes `main.rs`, inkl. `lprint` +
      `Err`-Helper + Unit-Tests),
   3. `ci: build and release 03_mvp_lisp` (Schritt 5),
   4. danach `walkthrough.md` schreiben (Spikes, Hatches, Transpiler-
      Lückenvorschläge mit Beispielen, umgesetzte Verbesserungen, Learnings,
      Erweiterungen, Docker-Programme) und als `docs(plan): add 03 walkthrough`
      committen.
3. ✅ `git status --short` zeigt nur beabsichtigte Dateien (`target/` nie dabei);
   `git log --oneline` zeigt die Commit-Reihe.
