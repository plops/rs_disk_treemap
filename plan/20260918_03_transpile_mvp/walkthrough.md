# Walkthrough: Transpilierung `00_mvp` → `03_mvp_lisp` (03_transpile)

Datum: 2026-09-18 · Stand: implementiert, verifiziert, committed.
Auftrag: wol pumba · Plan: `plan.md` (approved, inkl. Nachträgen `lprint`,
Release-CI, Fehlerausgabe statt `Err`-Ignoranz, Verbesserungsmandat).

## Was wirklich implementiert wurde

1. **`feat(03_mvp_lisp)`: Transpilat aus `gen.lisp`.** Neues Projekt
   `03_mvp_lisp/` (`Cargo.toml` handgeschrieben, `macroquad 0.4`, Lock 0.4.16;
   `src/main.rs` generiert, 373 Zeilen, committet). Generator-Quellen in
   kleinen Dateien statt einer großen (`gen.lisp` = Loader, 66 Zeilen;
   `lisp/helpers|scan|layout|render|main.lisp`, je 53–83 Zeilen, jede
   Emitter-Funktion ≤ 60 Zeilen, Top-Level-Closer auf eigenen Zeilen —
   Vorgabe aus der Review-Schleife, damit Klammern im Editor prüfbar bleiben).
2. **`lprint` mit C++-Semantik** (`helpers.lisp`): `(lprint :msg "layout"
   :vars '(screen_w ...))` baut aus jedem Ausdruck via `(emit-rs :code e)`
   Label + `{:?}`-Wert in genau ein `eprintln!`; `(:as "label" expr)` für
   eigene Labels. Einsatzstellen: Start-Target, Layout-Kennzahlen.
   Repetitives via `,@(loop ...)`-Splices (`ext-is`-Farbgruppen,
   `format_bytes`-Testdaten).
3. **Faktorisierte Fehlerausgabe** (statt stiller `Err`-Ignoranz, auf deinen
   Einwand): genau eine generierte Helper-`fn`
   `report_skip(context: &str, path: &Path, err: std::io::Error)` mit einem
   `eprintln!`-Ort; alle vier Fehlerstellen (`read_dir`, Eintrags-Iteration,
   `file_type`, `metadata`) rufen sie einzeilig auf. Verifiziert:
   `skip [read_dir]: /nonexistent-xyz: Os { code: 2, ... }`, kein Panic,
   GUI läuft weiter.
4. **CL-nahe Umformulierungen** (kein Text-Clone): `scan_entry` als eigene
   `fn` extrahiert; `worst`/`layout_row` als Top-Level-Funktionen mit
   expliziten `areas`-Parametern (statt Closures mit Captures);
   `for i in 0..areas.len()` statt `enumerate`-Destrukturierung;
   ungenutztes `Receiver`-Import entfallen; generierte `#[test]`-Unit-Tests
   für `format_bytes` (Muster `gen00.lisp`), 4 Fälle via Splice generiert.
5. **`ci`: `03` in Release-CI.** `build.yml`: Matrix-Eintrag
   (`dir: 03_mvp_lisp`, `bin: treemap_lisp`, Linux + Windows inkl.
   `v*`-Tag-Release), Smoke-Job um `03`-Fmt/Clippy/Test und GUI-Smoke mit
   `lprint`-Grep erweitert. YAML per `yaml.safe_load` validiert, alle neuen
   Schritte lokal ausgeführt.
6. **`docs(plan)`: Plan-Dokumente + Beispiele.** `plan.md`, `task.md`,
   `deps.md` (keine neue Dep, `macroquad` 0.4.16 = `cargo search`-Newest),
   `prompt.txt` und `gen-cpp-freestanding-example.lisp` committet.

Ergebnis: `cargo build`, `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`, `cargo test` (1 Unit-Test) grün; Transpiler-Suite
173/173 grün; beide GUIs unter `xvfb-run` Exit 124; `cargo build --release`
grün.

## Testbedingte Änderungen und Messungen

- **`stmt`-Hüllung:** Tail-`=` ohne Semikolon (`last_size = ...`,
  `avail_h = ...`) bricht `if`-Arme typmäßig; `draw_text` gibt
  `TextDimensions` (nicht `()`) zurück → beide Stellen plus beide
  `draw_text`-Tails mit Transpiler-`(stmt ...)` (Beleg: `stmt`-Test) geheilt.
- **Tail-`return` gestrichen (5×):** `needless_return`-Warnungen —
  generierte Fns enden jetzt mit bloßen Ausdrücken (`node`, `hash_color(..)`).
- **Shorthand-Feld:** `size: size` → bloßes `size` (`redundant_field_names`).
- **`f32/f64`-Casts:** `sum / side` → `sum / (side as f64)` (Referenz macht
  es implizit an derselben Stelle; hier explizit wegen `&[f64]`-Signatur).
- **`(/= d 1024.0)` → String-Hatch `"d /= 1024.0;"`:** Der Emitter schreibt
  `d/=(1024.0)`, `rustfmt` behält die redundanten Klammern
  (`unused_parens`) — als Upstream-Vorschlag unten notiert.
- **`move`-Closure per String-Hatch:** `(thread--spawn "move || { ... }")`
  (der Transpiler dokumentiert das selbst so: „use the `move' escape hatch“,
  s. `parse-lambda`-Kommentar).
- **`channel::<Node>()` per Turbofish-Call:** `((scope channel (angle Node)))`
  → `(channel::<Node>)()` — nötig, weil `declare` keine Tupel-Pattern
  annotieren kann (Upstream-Vorschlag unten).
- **Diff `00_mvp` vs. Generat:** 543 Diff-Zeilen, alle erklärt (Helper,
  Extrakt-Refactors, Logging, Tests, `0.150`-Float-Druck, Transpiler-Klammern).
  Kein unbeabsichtigter Verhaltensunterschied gefunden.

## Verifikation der Szenarien

- `sbcl --load 03_mvp_lisp/gen.lisp --quit` → Exit 0, `src/main.rs` neu
  geschrieben (nach `*rustfmt-arguments*`-Fix `--edition 2024`; ohne ihn
  scheitert `rustfmt` an `async fn` mit Edition-2015-Default).
- Fehler-Fixture (nicht-existenter Pfad): eine `skip`-Zeile pro Stelle,
  kein Panic, Exit-Verhalten wie Referenz (leerer Baum).
  Hinweis: `chmod 000` als root bedeutungslos (Container läuft als root;
  wie in `02` dokumentiert, entfällt dieser Fixture-Fall).
- `xvfb-run -a timeout 15 ./target/debug/treemap_lisp <dir>` → Exit 124,
  stderr zeigt `target ...`- und `layout ...`-Zeilen; Referenz parallel
  Exit 124 bei leerer stderr.
- Transpiler-Regression `run-tests.sh`: 173/173 (unverändert grün).

## Learnings

- **Klammer-Disziplin:** Die Datei wurde mehrfach „balanciert, aber falsch
  verschachtelt“ (Reader-OK, Emitter falsch: `let return = ...`,
  `draw_text` als Binding). Wirksamste Gegenmittel: Dateien ≤ ~80 Zeilen,
  Emitter ≤ 60 Zeilen, Top-Level-Closer einzeln — plus der
  Struktur-Probe (`decls-count`/`body-count` des evaluierten Templates),
  die den Fehler in Sekunden lokalisiert, wo Zählen versagt.
- **`return` am Fn-Ende:** Der Transpiler emittiert `(return X)` immer mit
  `return`; am Tail warnt Clippy. Konvention: nur frühe Abbrüche mit
  `return`, Tails als bloße Ausdrücke.
- **`eprintln!`-Inline-Captures:** `{err:?}` im Format-String macht ein
  zusätzliches `err`-Argument zum „redundant argument“-Fehler — ein
  Argument weniger als man denkt.
- **Glob-`use`, `attr`, `case`-`t`, `if-let`/`let-else`, `vec!`, `stmt`,
  `ref`/`ref-mut`/`deref`, `scope`-Turbofish:** alle belegt und im
  Transpilat im Einsatz; `SUPPORTED_FORMS.md` trug jede Entscheidung.

## Mögliche Erweiterungen (bewusst offengelassen)

0-Byte-Filter sichtbar machen (Zähler/`lprint`-Summary); `size > 0`-Regel
für Verzeichnisse lockern; `Crate` später auf `01_more`-Stand heben;
`notify`-Watch auch im Transpilat (derzeit Single-Scan wie `00_mvp`).

## Transpiler-Lückenvorschläge (mit Beispiel-Anwendung)

1. **`move`-Closures:** `(lambda (move) (...) ...)` oder
   `(move (lambda ...))` statt String-Hatch.
   Anwendung: `(thread--spawn (move (lambda () ...)))` →
   `thread::spawn(move || { ... })`.
2. **Tupel-Pattern in `declare`:** `(declare (type "(A, B)" (tuple x y)))`
   stürzt heute im `assert` ab (`variable-declaration`); Workaround war der
   Turbofish-Call. Anwendung: typisierte
   `let (tx, rx): (Sender<Node>, Receiver<Node>) = ...`-Destrukturierung.
3. **Assign-Op-Klammern:** `(/= d 1024.0)` sollte `d /= 1024.0` (ohne
   `(...`) emittieren — analog zu den bestehenden `omit-*`-Tests
   (`omit-reciprocal`, `omit-question`); heute nötig: String-Hatch.
4. **`return`-Am-Tail:** Ein `omit-return-tail`-Modus (oder Doku-Hinweis),
   der Tail-`(return X)` zu `X` macht, würde `needless_return`-Fixläufe
   nach jeder Generierung ersparen.

## Neu in den Docker-Container aufzunehmende Programme

- `sbcl` (2.6.0 hier) + Quicklisp mit `cl-rust-generator` als
  Local-Project (`~/quicklisp/local-projects/`); dringend: die
  `ql:register-local-projects`-Pflicht dokumentieren (bloßes `quickload`
  scheitert mit `SYSTEM-NOT-FOUND`).
- `rustfmt`-Komponente der Toolchain (das Generat wird automatisch
  formatiert; mit `*rustfmt-arguments* '("--edition" "2024")'`).
- Unverändert aus `02`: `xvfb` + `libxkbcommon0 libxi6 libx11-6 libgl1
  libasound2t64` für Build und GUI-Smoke.
