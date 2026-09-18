# deps.md — Abhängigkeiten `treemap-disk-analyzer` (01_more)

Es wurden **keine neuen Abhängigkeiten** eingeführt (Vorgabe: Code und Deps
minimal halten). Stand: `01_more/Cargo.toml`, `01_more/Cargo.lock`.

## Bestehende Dependencies (für spätere deepwiki-Abfragen)

| Crate (Version s. Cargo.lock) | GitHub-Org/Repo | deepwiki-Vorschlag |
|---|---|---|
| `macroquad 0.4` (GUI + `miniquad`-Backend, Fenster, Text) | `not-fl3/macroquad` | `not-fl3/macroquad` |
| `miniquad 0.4` (transitiv über macroquad; dort sitzt `XOpenDisplay`) | `not-fl3/miniquad` | `not-fl3/miniquad` |
| `rayon 1.10` (im Manifest; parallele Auswertung) | `rayon-rs/rayon` | `rayon-rs/rayon` |
| `notify 6.1` (Dateisystem-Watcher, `macos_kqueue`-Feature, `default-features = false`) | `notify-rs/notify` | `notify-rs/notify` |

## Hinweise

- Versions-Update-Policy aus dem Auftrag („bei neu eingeführten Deps immer
  neueste Version“) griff nicht, da keine Dep eingeführt wurde. `cargo build`
  löste innerhalb der `Cargo.toml`-Ranges auf (u. a. macroquad 0.4.16,
  notify 6.1.1); kein `cargo update` mit Sprung über Major-Grenzen.
- `notify 6.x` ist veraltet (aktuell 7+/8); bewusst nicht angehoben, um den
  Fix-Churn klein zu halten. Kandidat für einen separaten `chore(deps)`-Schritt.
- Test-/Lint-Tooling kommt aus der Toolchain (`cargo test/fmt/clippy`),
  keine Dev-Dependencies nötig. Integrationstest nutzt
  `std::process::Command` + Temp-Fixture, kein `tempfile`/`assert_cmd`.
