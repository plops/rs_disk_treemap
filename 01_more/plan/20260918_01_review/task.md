# task.md — Serielle Implementierungs- und Testschritte

Jeder Schritt endet mit Validierung; erst bei Grün weiter. Arbeitsverzeichnis
für `cargo`-Befehle: `01_more/`.

## Schritt 1 — Reproduktion (ohne GUI/X-Server)

1. Minimal-Spike nach `/tmp/repro_slice.rs` schreiben, der exakt die alte
   Abschneidelogik (`&label[..max_chars - 2]` mit `"abc还def"`, `max_chars=7`)
   ausführt.
2. `rustc -o /tmp/repro_slice /tmp/repro_slice.rs && /tmp/repro_slice`
3. ✅ Erwartet: Panic `end byte index 5 is not a char boundary; it is inside
   '还' (bytes 3..6 ...)` — identisch zur Meldung aus dem Report.

## Schritt 2 — Baseline sichern

1. `cargo build` (Dep-Fetch beim ersten Mal) muss grün sein.
2. `grep -n 'label\[..' src/main.rs` → einzige Byte-Slice-Stelle finden
   (`render_treemap`).

## Schritt 3 — Unicode-Fix implementieren

1. `pub fn truncate_label(label: &str, max_chars: usize) -> String` in
   Abschnitt 6 einfügen (Char-Grenzen via `char_indices`, Char-Count statt
   Byte-Länge, `max_chars <= 3` → Passthrough wie bisher).
2. Aufrufstelle in `render_treemap` ersetzen.
3. ✅ `cargo build` grün.

## Schritt 4 — Headless-Modus implementieren

1. `#[macroquad::main] async fn main` → `async fn gui_main` (Body unverändert,
   Target-Auflösung über neuen Helper `resolve_target`).
2. Neu (Abschnitt 8): `target_from_args`, `resolve_target`,
   `has_graphical_display` (Linux: `DISPLAY`/`WAYLAND_DISPLAY`),
   `run_headless` (Scanner/ Merge/ Statistik wiederverwenden, sortierte
   stdout-Ausgabe, Exit-Code als Rückgabe).
3. Neues `fn main`: `--headless`/`--scan-only` oder kein Display →
   `run_headless`; sonst `macroquad::Window::from_config(window_conf(),
   gui_main())` (= exakte Expansion von `#[macroquad::main]`, belegt in
   `macroquad_macro-0.1.8/src/lib.rs`).
4. ✅ `cargo build` grün.

## Schritt 5 — Tests schreiben und ausführen

1. Unit-Tests (`#[cfg(test)] mod tests` am Ende von `src/main.rs`):
   ASCII, CJK-Crashfall (`"abc还def"`), Emoji/schmale Breiten,
   Flag-Parsing.
2. Integrationstest `tests/headless_scan.rs`: Fixture mit CJK-Top-Level-Datei
   bauen, Binary mit `--headless` starten, Exit 0 + Inhalte + Totals prüfen.
3. ✅ `cargo test` → 4 Unit + 1 Integration grün.
4. ✅ `cargo fmt --check` sauber (`cargo fmt` bei Diffs).
5. ✅ `cargo clippy --all-targets`: keine **neuen** Warnungen (1×
   `unnecessary_sort_by` an Bestand-Zeile ist vorbekannt, nicht anfassen).

## Schritt 6 — Manuelle Szenario-Verifikation (Report-Fälle)

1. `env -u DISPLAY -u WAYLAND_DISPLAY ./target/debug/treemap-disk-analyzer
   /tmp/fakehome` (mit CJK-Dateiname) → ✅ Exit 0, Hinweis + Scan statt
   `XOpenDisplay`-Panic.
2. `./target/debug/treemap-disk-analyzer --headless /tmp/fakehome` → ✅ Exit 0,
   identische Ausgabe ohne Hinweis.
3. GUI-Pfad auf Maschine mit X-Server gegenprüfen (im Docker ohne X nicht
   möglich; Codepfad ist bis auf den `Window::from_config`-Aufruf unverändert).

## Schritt 7 — Dokumente schreiben

`plan.md`, `task.md` (diese Datei), `deps.md`, `walkthrough.md` in
`01_more/plan/20260918_01_review/` ablegen.

## Schritt 8 — Commits (Conventional Commits, nur eigene Pfade!)

1. `git status --short` prüfen; `target/`-Verzeichnisse **nie** stagen.
2. Commit 1 (`fix`): `01_more/src/main.rs`,
   `01_more/tests/headless_scan.rs`, `01_more/Cargo.lock` (Binary-Lockfile).
3. Commit 2 (`docs`): `01_more/plan/20260918_01_review/*.md`.
4. Body je Commit: Was/Warum/beobachtete Testergebnisse.
