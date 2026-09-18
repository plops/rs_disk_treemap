# deps.md — Abhängigkeiten `00_mvp` + `01_more`

Es wurden **keine neuen Abhängigkeiten** eingeführt (Vorgabe: Code und Deps
minimal halten). Ein Kandidat wurde im Gegenteil zum Entfernen vorgemerkt.
deepwiki-MCP war in diesem Container nicht verfügbar; die `deepwiki`-Spalte
ist so gewählt, dass Abfragen später direkt konstruierbar sind
(`plops/rs_disk_treemap` für das Repo, `ORG/REPO` pro Crate).

## Bestehende Dependencies

| Crate (Stand s. Cargo.lock) | Verwendet in | GitHub-Org/Repo | deepwiki-Vorschlag |
|---|---|---|---|
| `macroquad 0.4` (Lock 0.4.16 = neueste 0.4.x) | 00_mvp, 01_more (Fenster, Text, Input) | `not-fl3/macroquad` | `not-fl3/macroquad` |
| `miniquad 0.4` (transitiv; dort sitzt `XOpenDisplay`/`Window::from_config`) | beide (transitiv) | `not-fl3/miniquad` | `not-fl3/miniquad` |
| `notify 6.1` (Lock 6.1.1; `default-features = false`, Feature `macos_kqueue`) | 01_more (Live-Watcher) | `notify-rs/notify` | `notify-rs/notify` |
| `rayon 1.10` (Lock-Verbund bis 1.12) — **ungenutzt, zum Streichen vorgemerkt** | 01_more (deklariert, kein Treffer in `src/`) | `rayon-rs/rayon` | `rayon-rs/rayon` |

## Neueste-Version-Check (rust-Tools, Stand 2026-09-18)

- `cargo search macroquad` → `0.4.16` = Lock-Stand → kein Update nötig.
- `cargo search rayon` → `1.12.0`; irrelevant, Dep wird entfernt statt upgedatet.
- `cargo search notify` → `9.0.0-rc.5` (Pre-Release), letztes stabiles
  **8.2.0** (verifiziert via docs.rs). Upgrade 6.1 → 8 ist breaking und läuft
  als separater `chore(deps)`-Schritt mit Migrationsbeispiel (s. `plan.md §7`),
  nicht in diesem Plan.
- Versions-Update-Policy aus dem Auftrag („bei neu eingeführten Deps immer die
  neueste Version“) griff nicht, da keine Dep eingeführt wurde.

## Hinweise

- Beide `Cargo.lock` sind derzeit **untracked** (`git ls-files` ohne Treffer);
  als Schritt 2 von `task.md` einchecken (Binary-Crate-Konvention).
- Test-/Lint-Tooling kommt aus der Toolchain
  (`cargo test/fmt/clippy`, stabil 1.98.1), keine Dev-Dependencies nötig.
  Integrationstests nutzen `std::process::Command` + Temp-Fixture
  (kein `tempfile`/`assert_cmd`).
- Systemseitig (keine Cargo-Deps, aber für Docker/CI relevant):
  `xvfb`/`xvfb-run`, `libx11`, `libxi`, `libgl1-mesa`, `libasound2`,
  `libxkbcommon0` (Details s. `plan.md §9`).
