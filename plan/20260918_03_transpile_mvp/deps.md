# deps.md — Abhängigkeiten `03_mvp_lisp` (final, Stand 2026-09-18)

Vorgabe: Code und Deps minimal halten; bei neu eingeführten Deps immer die
neueste Version (`cargo search`) + Usage-Example. deepwiki-MCP war in diesem
Container nicht verfügbar; die `deepwiki`-Spalte ist so gewählt, dass Abfragen
später direkt konstruierbar sind (`plops/rs_disk_treemap` für das Repo,
`ORG/REPO` pro Crate).

## Dependencies (verifiziert)

Es wurde **keine neue Abhängigkeit** eingeführt — das Transpilat nutzt exakt
die eine Dep der Referenz (`macroquad`), Ersatz-Kandidaten fielen im Spike
durch (keine Vereinfachung belegt, s. Walkthrough).

| Crate (Stand s. `03_mvp_lisp/Cargo.lock`) | Verwendet in | GitHub-Org/Repo | deepwiki-Vorschlag |
|---|---|---|---|
| `macroquad 0.4` (Lock 0.4.16) | `03_mvp_lisp` (Fenster, Text, Input) | `not-fl3/macroquad` | `not-fl3/macroquad` |
| `miniquad 0.4` (transitiv) | `03_mvp_lisp` (transitiv) | `not-fl3/miniquad` | `not-fl3/miniquad` |

## Neueste-Version-Check (rust-Tools, Stand 2026-09-18)

- `cargo search macroquad` → `0.4.16` = Lock-Stand (`00_mvp`-Lock ebenfalls
  0.4.16) → kein Update nötig, kein Usage-Example erforderlich.
- Ersatz-Dep-Evaluierung (Plan-Entscheidung 9): kein Kandidat mit
  Vereinfachungs-Beleg gefunden; `macroquad` bleibt (bewährter `02`-Stand).

## Hinweise

- `sbcl` + `cl-rust-generator` (Quicklisp-Local-Project, `~/quicklisp/local-projects/`)
  sind Generator-Tooling, keine Cargo-Deps; `rustfmt` formatiert das Generat
  (`*rustfmt-program*`).
- Test-Tooling aus der Toolchain (`cargo test/fmt/clippy`), keine Dev-Deps.
  Generierte Unit-Tests (`#[cfg(test)]` in `main.rs`, Muster `gen00.lisp`)
  brauchen ebenfalls keine neue Dep.
- Systemseitig (Docker/CI): `xvfb`/`xvfb-run`, `libx11`, `libxi`, `libgl1-mesa`,
  `libasound2`, `libxkbcommon0` (Details s. Walkthrough nach Umsetzung).
