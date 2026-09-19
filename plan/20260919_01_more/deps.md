# deps.md — Abhängigkeiten `04_more_lisp` (Stand 2026-09-19)

Vorgabe: Code und Deps minimal halten; bei neu eingeführten Deps immer die
neueste Version (`cargo search`) + Usage-Example. deepwiki-MCP war in diesem
Container ohne Transport nicht verfügbar; die `deepwiki`-Spalte ist so gewählt,
dass Abfragen später direkt konstruierbar sind (`plops/rs_disk_treemap` für das
Repo, `ORG/REPO` pro Crate).

## Dependencies (Zielstand: keine neue Abhängigkeit)

Das Transpilat nutzt exakt die zwei Deps der Referenz `01_more`:

| Crate (Stand) | Verwendet in | GitHub-Org/Repo | deepwiki-Vorschlag |
|---|---|---|---|
| `macroquad 0.4` (Lock 0.4.16) | `04_more_lisp` (Fenster, Text, Input, Kamera) | `not-fl3/macroquad` | `not-fl3/macroquad` |
| `miniquad 0.4` (transitiv) | `04_more_lisp` (transitiv) | `not-fl3/miniquad` | `not-fl3/miniquad` |
| `notify 6.1` (`default-features = false`, `macos_kqueue`) | `04_more_lisp` (Live-Watcher) | `notify-rs/notify` | `notify-rs/notify` |

## Neueste-Version-Check (rust-Tools, Stand 2026-09-19)

- `cargo search macroquad` → `0.4.16` = Lock-Stand (`01_more`-Lock ebenfalls
  0.4.16) → kein Update nötig, kein Usage-Example erforderlich.
- `cargo search notify` → newest `9.0.0-rc.5`, Referenz pinnt `6.1`. **Kein
  Upgrade im Transpilat:** 6→9 ist ein breaking API-Wechsel (u. a. Debouncer- und
  Backend-Umstellung), der eine eigene Evaluierung mit Spike verdient (s. plan.md,
  Vorschlag 5). `default-features = false` + `macos_kqueue` bleiben wie in der
  Referenz erhalten (kleine Binärgröße, Vorgabe aus dem Auftrag).
- Ersatz-Dep-Evaluierung (Plan-Entscheidung 9): kein Kandidat mit
  Vereinfachungs-Beleg; `macroquad` + `notify` bleiben.

## Usage-Examples (vorab, für den ausführenden Agenten)

`notify 6.1` (`RecommendedWatcher`, Kanal-Brücke ins GUI — so nutzt es die Referenz):

```rust
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::channel;
let (tx, rx) = channel();
let mut watcher: RecommendedWatcher =
    notify::recommended_watcher(move |res| { let _ = tx.send(res); })?;
watcher.watch(path, RecursiveMode::NonRecursive)?;
while let Ok(res) = rx.try_recv() { /* EventKind filtern, Baum updaten */ }
```

`macroquad 0.4` (Fenster-Einstieg ohne `#[macroquad::main]`-Attr — nötig, weil die
Headless-Weiche vor `XOpenDisplay` laufen muss):

```rust
macroquad::Window::from_config(window_conf(), gui_main());
// async fn gui_main() { ... next_frame().await; }
```

Transpiler-Seite (`scope`/`angle`-Turbofish, Muster `03`):

```lisp
((tuple tx rx) ((scope channel (angle Node))))
(thread--spawn "move || { ... }")
```

## Hinweise

- `sbcl` + `cl-rust-generator` (Quicklisp-Local-Project, `~/quicklisp/local-projects/`)
  sind Generator-Tooling, keine Cargo-Deps; `rustfmt` formatiert das Generat
  (`*rustfmt-arguments* '("--edition" "2024")`).
- Test-Tooling aus der Toolchain (`cargo test/fmt/clippy`), keine Dev-Deps.
  Generierte Unit-Tests (`#[test]` in `main.rs`, Muster `gen00.lisp`-`test_parse_pair`)
  und `tests/headless_scan.rs`-Port brauchen ebenfalls keine neue Dep.
- Systemseitig (Docker/CI): `xvfb`/`xvfb-run`, `libx11-dev`, `libxi-dev`,
  `libgl1-mesa-dev`, `libasound2-dev`, `libxkbcommon0` (Details s. Walkthrough
  nach Umsetzung). Befund 2026-09-19: `xvfb-run` fehlt im Container —
  Installation (`apt-get install -y xvfb ...`) oder Smoke-Verlagerung in die CI.
