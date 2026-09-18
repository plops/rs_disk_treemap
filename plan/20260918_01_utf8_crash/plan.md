# Implementierungsplan: Crash-Fixes `treemap-disk-analyzer` (01_more)

Stand: 2026-09-18 · Auftrag: wol pumba · Code/Abhängigkeiten minimal halten

## 1. Befund (zwei unabhängige Abstürze)

1. `XOpenDisplay() failed!` (miniquad 0.4, `linux_x11.rs`): kein X-Server im
   Docker-Container / auf Servern. `#[macroquad::main]` öffnet das Display,
   **bevor** eigener Code läuft — daher keine Chance auf saubere Fehlermeldung.
2. `end byte index 5 is not a char boundary; it is inside '还' (bytes 3..6)`:
   `render_treemap` schnitt Labels per Byte-Index (`&label[..max_chars - 2]`)
   ab. Jeder mehrbytige Dateiname (CJK, Emoji, Umlaute in manchen
   Normalisierungen) lässt den GUI-Thread paniken, sobald die Zelle schmal
   genug wird.

## 2. Lösungsdesign (ohne neue Abhängigkeiten)

- **Unicode-Fix:** neue Funktion `truncate_label(label, max_chars)` schneidet
  an Char-Grenzen (`char_indices().nth(...)`) und vergleicht Zeichen- statt
  Byte-Länge. Einzige Aufrufstelle: `render_treemap`.
- **Headless-Modus:** `#[macroquad::main]` wird ersetzt durch ein eigenes
  `fn main()`, das erst prüft und dann `macroquad::Window::from_config(...)`
  aufruft — exakt das, wozu das Makro expandiert (verifiziert in
  `macroquad_macro-0.1.8/src/lib.rs`). Dadurch läuft die Display-Prüfung
  **vor** `XOpenDisplay()`:
  - `--headless` / `--scan-only` → Textmodus (immer).
  - Linux ohne `DISPLAY` und ohne `WAYLAND_DISPLAY` → Textmodus mit Hinweis
    auf stderr (inkl. `xvfb-run`-Tipp für die GUI).
  - sonst → GUI wie bisher, unveränderter `window_conf()` + Async-Body.
- **Textmodus** nutzt Scanner (`scan_directory_recursive`), Merge-Logik
  (`merge_scanned_node`, `find_mut`) und Statistik (`sum_tree_stats`) der GUI
  wieder — kein Doppel-Code. Ausgabe: Totalsumme + größensortierte
  Top-Level-Einträge (`name/` für Verzeichnisse).

## 3. Requirements-Abgleich

Explizit gefordert: Absturzursachen beheben, auch ohne X-Server reproduzier-
und debuggbar, Code/Deps klein, rust-Tools (fmt/clippy), Tests + Ausführung,
plan/task/deps/walkthrough-Dokumente, Conventional Commits.

Nicht explizit genannt, aber umgesetzt bzw. vorgeschlagen:

- Umgesetzt: `--headless`-Flag (skriptfähig, CI-fähig), Auto-Fallback mit
  verständlicher Meldung statt Panic, `xvfb-run`-Hinweis.
- Vorschlag (nicht umgesetzt, bewusst zurückgestellt): `--help`-Ausgabe,
  Tiefe/Begrenzung für Headless (`--depth`, `--top`), JSON-Ausgabe für
  Weiterverarbeitung, GUI-seitig CJK-sichere Breitenmessung statt
  7.2-px-Schätzung, `notify`-Version anheben (6.x ist alt; wegen
  „keine neuen Deps/kein Churn“ nicht angefasst).

## 4. Kontext-Dateien für einen unabhängigen Agenten

| Datei | Wozu lesen |
|---|---|
| `01_more/src/main.rs` | Gesamte App (~1300 Zeilen, einzige Quelldatei): Abschnitte 1 Tree-Modell, 2 Squarified-Layout, 3 Scanner, 4 Watcher, 5 Rendering, 6 Formatierung (`truncate_label`), 7 GUI-Loop (`gui_main`), 8 Headless-Modus, Tests am Dateiende |
| `01_more/Cargo.toml` | 3 Deps (macroquad, rayon, notify) + Release-Profil (`panic="abort"` beachten) |
| `01_more/Cargo.lock` | Eingecheckt (Binary-Crate-Konvention); pinnnt u. a. macroquad 0.4.x |
| `01_more/tests/headless_scan.rs` | Integrationstest: baut Fixture (inkl. CJK-Name), startet Binary mit `--headless`, prüft Exit-Code + stdout |
| `01_more/plan/20260918_01_review/task.md` | Serielle Arbeits-/Testschritte |
| `01_more/plan/20260918_01_review/deps.md` | Abhängigkeiten + GitHub-Orgs (deepwiki-Abfragen) |
| `01_more/plan/20260918_01_review/walkthrough.md` | Was wirklich umgesetzt wurde + Learnings |
| `~/.cargo/registry/src/*/macroquad_macro-0.1.8/src/lib.rs` | Beleg, dass `Window::from_config(conf, fut)` die Makro-Expansion ist |
| `~/.cargo/registry/src/*/macroquad-0.4.14/src/lib.rs` (Zeilen ~870–900) | `Window::from_config`-Signatur |

Hinweis: deepwiki-MCP (`plops/rs_disk_treemap`) stand in diesem Container
nicht zur Verfügung; stattdessen lokale Registry-Quellen + `cargo`-Tools.
Kein Internet-Zugriff nötig außer beim erstmaligen `cargo build` (Dep-Fetch).

## 5. Commit-Konvention (Conventional Commits, ausführliche Bodies)

- Format: `<type>(01_more): <kurze Imperativ-Zusammenfassung>` + Leerzeile +
  Body (Was/Warum/Tests) + ggf. `Refs:`-Zeile.
- Types: `fix` (Crash-Fixes + Tests), `docs` (plan-Dokumente),
  `test`/`refactor` nur bei Bedarf; ein Commit = ein Type.
- Body-Sprache: Deutsch oder Englisch, konsistent pro Commit; Dateinamen +
  beobachtete Testausgaben (`cargo test`, `cargo clippy`, `cargo fmt --check`)
  nennen. Kein `git add -A` — nur eigene Pfade stagen (`target/` ist
  absichtlich nicht versioniert und bleibt draußen).
- Beispiel:
  `fix(01_more): prevent unicode label panic and support headless scans`
