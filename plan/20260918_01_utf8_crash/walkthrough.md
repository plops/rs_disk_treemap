# Walkthrough: Crash-Fixes `treemap-disk-analyzer` (01_more)

Datum: 2026-09-18 · Stand: Fix + Tests implementiert, verifiziert, committed.

## Was wirklich implementiert wurde

`01_more/src/main.rs` (einzige Quelldatei, keine neuen Deps):

1. **Unicode-Fix** — `truncate_label()` (Abschnitt 6) schneidet Labels an
   Char-Grenzen ab und zählt Zeichen statt Bytes. Ersetzt exakt eine Stelle in
   `render_treemap`. Der Report-Crash (`'还'`, Bytes 3..6) ist damit
   unmöglich; Repro-Spike unter `/tmp/repro_slice.rs` belegte vorher die
   byte-identische Panic.
2. **Headless-Modus** (Abschnitt 8) — neues `fn main()` prüft *vor* jeder
   GUI-Initialisierung: `--headless`/`--scan-only` oder (Linux) weder
   `DISPLAY` noch `WAYLAND_DISPLAY` → `run_headless()` druckt Totalsumme +
   größensortierte Top-Level-Einträge und exitet 0. Sonst startet die
   unveränderte GUI über `macroquad::Window::from_config` (nachgewiesen die
   exakte `#[macroquad::main]`-Expansion). `run_headless` nutzt Scanner,
   Merge- und Statistik-Funktionen der GUI wieder.
3. **Tests** — 4 Unit-Tests in `src/main.rs` (ASCII, CJK-Crashfall, Emoji,
   Flag-Parsing) + `tests/headless_scan.rs` (End-to-End mit CJK-Fixture,
   Exit-Code, stdout-Totals). `cargo test`: 5/5 grün. `cargo fmt --check`:
   sauber. `cargo clippy --all-targets`: keine neue Warnung (eine
   vorbestehende `unnecessary_sort_by`-Warnung an Alt-Code bewusst belassen).

## Testbedingte Änderungen (Abweichungen vom ersten Wurf)

- Emoji-Test: `"a🦀bc"` mit `max_chars=3` erwartet — Design gibt Breiten ≤ 3
  unverändert zurück (alter `max_chars > 3`-Guard). Test auf `"a🦀bcd"/4 →
  "a🦀.."` korrigiert statt Produktcode aufzuweichen.
- Integrationstest-Fixture: CJK-Datei lag zuerst nur verschachtelt (`sub/`),
  Headless listet aber Top-Level. Zusätzliche Top-Level-CJK-Datei
  (`Bericht-还.txt`) ergänzt; Assertions auf 3 Dateien / 180 Bytes angepasst.
- Eigener neuer Code nutzt `sort_unstable_by_key(Reverse(...))`, um keine
  zweite Clippy-Warnung einzuführen; Alt-Zeile unangetastet (minimaler Diff).

## Verifikation der Report-Szenarien

- `env -u DISPLAY -u WAYLAND_DISPLAY ./target/debug/treemap-disk-analyzer
  /tmp/fakehome` → Exit 0, stderr-Hinweis (inkl. `xvfb-run`-Tipp), korrekter
  Scan mit CJK-Name. Kein `XOpenDisplay`-Panic mehr.
- `--headless`-Variante → identische Ausgabe ohne Hinweis, Exit 0.
- GUI-Pfad im Container ohne X-Server nicht ausführbar; Kompilat geprüft,
  Laufzeitpfad bis auf den belegten `Window::from_config`-Aufruf unverändert.

## Learnings

- `#[macroquad::main]` öffnet das Display vor eigenem Code — jede
  Display-Prüfung *innerhalb* des Async-Mains käme zu spät. Die Doku-versteckte,
  aber öffentliche `Window::from_config`-API löst das ohne neue Dep.
- `str::len()` vs. `chars().count()` + Byte-Slicing ist der klassische
  deutsch/CJK-Crash in Render-Code; gehört in jeden Label-Helper, nicht an die
  Aufrufstelle.
- Binary-Crates können Integrationstests ohne neue Deps schreiben
  (`current_exe()`-Ableitung statt `CARGO_BIN_EXE_*`-Raten + tempfile-Crates).

## Mögliche Erweiterungen (bewusst offengelassen)

`--help`, `--depth`/`--top`-Begrenzung und JSON-Ausgabe für Headless;
CJK-taugliche Breitenmessung statt 7.2-px-Schätzung; `notify 6.x → 7/8` als
separater `chore(deps)`-Schritt; GUI-Smoke-Test unter `xvfb-run` in CI.

## Neu in den Docker-Container aufzunehmende Programme

- `xvfb` (Paket `xvfb`, enthält `xvfb-run`) — einziger sinnvoller Zusatz:
  ermöglicht GUI-Start **und** GUI-Smoke-Tests (`xvfb-run -a
  ./treemap-disk-analyzer <dir>`) auf Servern/CI ohne echten X-Server.
  Installiert à la `apt-get install -y xvfb`.
- Sonst nichts: Fix, Tests und Verifikation kamen mit vorhandener
  Rust-Toolchain (`cargo build/test/fmt/clippy`, `rustc` für den Repro-Spike)
  aus; kein neues Analyse- oder Laufzeit-Tool nötig.
