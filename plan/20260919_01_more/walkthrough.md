# walkthrough.md — 01_more → 04_more_lisp (Stand 2026-09-19)

## Was umgesetzt wurde

`01_more` (`treemap-disk-analyzer`, 1440 Zeilen Rust) wurde per
`cl-rust-generator`-Transpiler nach `04_more_lisp/` überführt. Source of Truth
ist `04_more_lisp/gen.lisp` (Loader nach `03_mvp_lisp/gen0.lisp`-Muster) plus
zehn Emitter-Module in `04_more_lisp/lisp/` (`helpers, data, layout, scan,
watcher, format, headless, render, gui, tests, main` — jeder Emitter ≤ 60
Zeilen, Top-Level-Closer auf eigenen Zeilen). `sbcl --load
04_more_lisp/gen.lisp --quit` schreibt `04_more_lisp/src/main.rs` (1605
Zeilen, committet); `Cargo.toml` ist handgeschrieben (`macroquad 0.4`,
`notify 6.1` wie Referenz). Alle 01_more-Funktionen sind vorhanden
(Inventar-Diff: nur dokumentierte Umformulierungen, s. unten).
`lprint`-Laufzeitstellen: `scan-start`/`scan-done` (headless),
`layout [tree_files]/[tree_bytes]` (GUI). Fehler-Helper: `report_skip`
(Scan, 03-Parität) + 1:1-Watcher-`eprintln!`-Orte. Tests: 6 generierte
Unit-Tests (`#[test]` in `main.rs`) + portierte `tests/headless_scan.rs`
(CJK-Fixture, Broken-Pipe-Fall). CI: Matrix-Eintrag (`04_more_lisp`) und
Smoke-Job (fmt/clippy/test, Headless- + GUI-Smoke mit `layout`-Grep).

## Was getestet wurde (alles grün am 2026-09-19 im Container)

- `sbcl --load 04_more_lisp/gen.lisp --quit` → Exit 0, keine
  Undefined-Function-Hinweise.
- `cargo build`, `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings` → 0 Warnungen.
- `cargo test` → 6 Unit + 2 Integration grün (einmalige Flake s. Learnings).
- `sh /workspace/src/cl-rust-generator/run-tests.sh` → 173/173.
- Headless-Fixture (`hello.txt` 100 B, `Bericht-还.txt` 30 B, `sub/daten.bin`
  50 B) → `180.00 B in 3 files`, CJK intakt, Exit 0; Fehler-Fixture
  (nicht existenter Pfad) → `skip [read_dir]: ... NotFound`, Exit 0, kein Panic.
- `xvfb-run -a timeout 20 ./target/debug/treemap_more_lisp /tmp/hl` → Exit
  124, `layout [tree_files]=3 [tree_bytes]=180` auf stderr, kein Panic
  (Referenz-Binary parallel: ebenfalls 124, kein Panic).
- `cargo build --release` → grün.
- `python3 -c "import yaml; ..."` auf `.github/workflows/build.yml` → grün;
  alle neuen CI-Schritte wurden lokal simuliert (inkl. `grep -q "^layout "`).
- Inventar-Diff der `fn`-Namen Referenz vs. Generat: nur beabsichtigte
  Abweichungen (`update_at`, `native_recursive` ×2, `backend_label` ×4,
  `report_skip`, cfg-gedoppelte `is_virtual…`/`has_graphical_display`).

## Testgetriebene Änderungen (alle im Lisp, nie im Generat)

- `merge_scan_batch` nimmt `batch` by value (statt `&mut`) — kein
  `mut`-Pattern im `while let` nötig; beide Aufrufer generiert.
- `report_skip`-Verdrahtung an allen drei Scan-Fehlerstellen (03-Parität;
  Referenz schweigt dort — einzige stderr-Verhaltensänderung, beabsichtigt).
- `print_line`: `match` auf `e.kind()` statt let-Kette (collapsible_if).
- `window_conf`: `#[allow(clippy::field_reassign_with_default)]` (keine
  `..Default::default()`-Syntax emittierbar) + Feldzuweisung auf
  `Conf::default()`.
- Verschachtelungs-Regel: `let` ohne Rumpf emittiert `{ }` — Bindungen, die
  später gebraucht werden, müssen verschachtelt sein (Wrapper-Emitter in
  `gui.lisp`: `prelude/frame/layout/input/draw/text/status-wrap`).
- `char`-Literal via `(char /)` (single_char_add_str); echter Tab via
  buchstäblichem Tab in der Lisp-Quelle (Backslash wird escapt, `string-r`
  auch); `eprintln!()` für Leerzeilen (println_empty_string).
- `camera_zoom *= / /=` als exakte Statement-Hatches (assign-ops klammern →
  unused_parens; `x = x op y` → assign_op_pattern).
- `#[allow(clippy::collapsible_if)]` genau einmal, statement-lokal an der
  Removal-Verschachtelung (let-Ketten nicht emittierbar).

## Neue Transpiler-Erkenntnisse (über 03 hinaus)

- `(space move (lambda ...))` baut `move`-Closures als Call-Argument
  (`notify::recommended_watcher`); ganze `spawn`-Statements weiter als String.
- `attr` auf `defun` trägt `#[cfg]`/`#[allow]`/`#[cfg(test)]`; `attr` auf
  `let` scopet die Bindung weg (deshalb `native_recursive`-/
  `backend_label`-Helper statt cfg-Blöcken).
- Lifetime via String-Funktionsname (`"find_hovered_path<'a>"`) verifiziert.
- `matches!` mit Tupel-Pattern geht als
  `(matches! (dot event kind) (space (scope EventKind Access) "(_)"))`
  (rustfmt-normalisiert, rustc-geprüft); `case` für denselben Zweck bricht.
- Nested-Pattern im `while-let` (`Ok(ScanEvent::Batch(batch))`) geht.
- `let ((_ ...))` emittiert `let _ = ...;`.
- `make-instance` mit barem Symbol = Shorthand (`color` statt `:color color`);
  Keyword mit abweichendem Wert für Umbenennung (`:watcher watcher_opt`).
- Shorthand-Pflicht: `redundant_field_names` schlägt sonst unter `-D` an.
- `or`/`and`/`not` auch in Werteposition; `?` als `(? expr)`; `>>`/`<<`
  vorhanden; `char` via `(char x)`; `String` via `owned`-Helper
  (`(dot (string s) (to_string))`).
- `aref` klammert zusammengesetzte Indizes (`components[(1..)]` →
  unused_parens); Umgehung: iteratives `find_mut` (`?`-Operator),
  positionsbasiertes `update_at`, `split_at(...).0` statt `[..len-1]`,
  bounded `range` wo möglich.
- `range-from` klammert fest (`(1..)`); ebenfalls Umgehung wie oben.
- `if` als `let`-Wert, `if-let` mit Else als Wert, Tupel-`let`-Pattern und
  `(paren a b)`-Tails funktionieren; `stmt` rettet Unit-Tails (`draw_text`,
  `await`).
- `defun-async` + `(await (next_frame))` für `gui_main`; `Window::from_config`
  statt `#[macroquad::main]` (Headless-Weiche vor `XOpenDisplay`).
- Kein `pub` nötig (Single-Binary-Crate; Tests via gleichem Modul).
- `parenmedic diagnose` meldete einmal fälschlich auf `tests.lisp`
  (SBCL las fehlerfrei) — unabhängige Zweitprüfung (eigener
  Klammerzähler + SBCL-Form-Reader) hat sich bewährt.
- Emitter-Disziplin: Callees vor Caller definieren (sonst
  Undefined-Function-Hinweise); Splits per `,`-/`,@`-Splices sind
  generat-identisch beweisbar (`diff` leer).

## Umgesetzte Verbesserungen (gegenüber 01_more, alle begründet)

1. Scan-Fehler melden sich via `report_skip` (statt Schweigen) — 03-Parität,
   Plan-Vorgabe.
2. `format_bytes`-Genauigkeit `{:.2}` per generiertem Test festgenagelt
   (gegen 03-`{:.1}`-Gewohnheit).
3. `merge_scan_batch`-Signatur (by value) + `update_at`-Helper dokumentiert.
4. Alle Emitter ≤ 60 Zeilen (nachträglich zerlegt; Generat per `diff` als
   byte-identisch bewiesen).

## Vorschläge: Transpiler-Lücken mit Beispiel-Anwendung

1. `range-from`/`range` ohne Hüllklammern in `:primary`-Position, damit
   `(aref components (range-from 1))` → `components[1..]` statt
   `components[(1..)]` (heute: unused_parens; Umgehung s. oben).
2. Let-Ketten (`if let A && let B`) — heute `collapsible_if`-Umwege oder
   `allow`; Beispiel: Removal-Branch in `gui_main`.
3. Struct-Update-Syntax (`..Default::default()`) für `make-instance` —
   heute Feldzuweisung + `allow` (`window_conf`).
4. `move`-Lambda direkter (heute `space`-Trick, dokumentiert aber
   überraschend); `defenum` mit Daten-Varianten (heute String-Hatch für
   `ScanEvent::Batch`).
5. `while-let` mit `mut`-Bindung im Pattern (heute By-Value-Umbau).
6. Dokumentieren: `let` ohne Rumpf → Block (Scope-Falle für Generator-Neulinge).

## Erweiterungen (bewusst nicht umgesetzt)

`notify 9`-Evaluierung (breaking, eigener Change), paralleler Scan,
CJK-Breitenmodell, `HashMap`-Merge, i18n, 0-Byte-Zähler in der Summary,
Inotify-Limit-Konstante, Release-Entscheid.

## Docker-/Systemprogramme (Beleg: frischer Container, 2026-09-19)

- `apt-get update && apt-get install -y xvfb` → `/usr/bin/xvfb-run`
  (Smoke-Muster `02`-Walkthrough: `timeout`-Exit 124 = gesund).
- `apt-get install -y libxkbcommon0 libxi6 libx11-6 libgl1 libasound2t64`
  (sonst `X11 backend failed: LibraryNotFound(DlOpenError("libxkbcommon…"))`
  — trifft Referenz und Transpilat identisch).
- `sbcl` (2.6.0.debian) + Quicklisp-`cl-rust-generator` via
  `(ql:register-local-projects)` (ohne: `SYSTEM-NOT-FOUND`); `cargo`,
  `rustfmt`, `clippy`, `python3-yaml` für CI-Validierung.

## Reproduzieren

```sh
sbcl --load 04_more_lisp/gen.lisp --quit
cd 04_more_lisp
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
./target/debug/treemap_more_lisp --headless <dir>
timeout 20 xvfb-run -a ./target/debug/treemap_more_lisp <dir>   # Exit 124
```
