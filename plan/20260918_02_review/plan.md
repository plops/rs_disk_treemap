# Implementierungsplan: Review `00_mvp` + `01_more` und Konsolidierung

Stand: 2026-09-18 · Auftrag: wol pumba · Vorgabe: Code und Abhängigkeiten minimal halten

## 1. Architektur im Vergleich

| Aspekt | `00_mvp` (285 Zeilen, 1 Datei) | `01_more` (1303 Zeilen, 1 Datei) |
|---|---|---|
| Baumodell | `Node { path, size, is_dir, children, rect, color }` | `FileNode { name, is_dir, size_bytes, children, target/current_rect, Animationsfelder }` |
| Scanner | `scan_tree`, rekursiv, Sync-`fs::read_dir`, Symlinks übersprungen, läuft in 1 Bg-Thread | `scan_directory_recursive`, Batch-Kanal (`ScanEvent::Batch`), Merge via `find_mut`/`merge_scanned_node`, 1 Bg-Thread + dynamische Worker bei Watcher-Events |
| Layout | `squarify` (klassisch, sortiert bei jedem Aufruf) | `squarify_children` + `LayoutWorkspace` (wiederverwendete `areas`/`row`-Puffer, `is_sorted`-Flag, Debounce 100 ms) |
| Rendering | `render_tree` (flache Rechtecke) | `render_treemap` (Cushion-Bevel, Wachstumspuls, Kamera Pan/Zoom, Hover-Hit-Test) |
| Live-Update | nein | `WatcherManager` (notify, inotify-Pool unter Linux, Limit-Warnung) |
| Headless | nein → im Docker unbrauchbar (verifiziert, s. §2) | ja (`--headless`/`--scan-only`, Display-Auto-Fallback, `truncate_label`-Fix) |
| Tests | keine | 4 Unit + 1 Integration (`tests/headless_scan.rs`) |

Beide Crates duplizieren voneinander unabhängig: Squarified-Layout, Extension-Farbgebung,
`format_bytes`, Virtual-FS-Filter (`/proc`, `/sys`, `/dev`).

## 2. Verifizierte Tool-Befunde (diese Sitzung, Container ohne X)

- `cargo fmt --check`: beide Crates sauber.
- `cargo clippy`: je **eine** Vorwarnung, beide harmlos, beide zu fixen:
  `00_mvp/src/main.rs:123` `needless_range_loop`, `01_more/src/main.rs:221` `unnecessary_sort_by`.
- `cargo test` (01_more): 5/5 grün (4 Unit + 1 Integration).
- GUI-Start im Container unmöglich, zweifach belegt:
  `timeout 15 ./target/release/treemap_mvp /tmp` → Panic in miniquad
  (`X11 backend failed: LibraryNotFound(libxkbcommon...)`);
  zusätzlich kein X-Server (`DISPLAY` leer, kein `/tmp/.X11-unix`,
  `Xvfb`/`xvfb-run` nicht installiert).
- `rayon 1.10` steht in `01_more/Cargo.toml`, wird aber in `src/main.rs`
  **nirgends verwendet** (`search(rayon|par_)` ohne Treffer) → tote Abhängigkeit.
- `git ls-files` enthält **keine** `Cargo.lock`-Datei (Binary-Crates sollten sie einchecken).
- deepwiki-MCP (`plops/rs_disk_treemap`) stand in diesem Container nicht zur
  Verfügung (kein MCP-Transport); Ersatzquellen: lokale Cargo-Registry,
  `cargo search`, docs.rs, GitHub-Web. deepwiki-Schlüssel stehen trotzdem in
  `deps.md`, damit spätere Abfragen direkt konstruierbar sind.

## 3. Kernbefunde (priorisiert)

1. **Tote Abhängigkeit:** `rayon` in `01_more` entfernen (keine Nutzung, Scan ist
   sequenziell). Alternative „Scan mit rayon parallelisieren“ verworfen: mehr
   Code/Komplexität gegen die Klein-halten-Vorgabe; der Bg-Thread reicht.
2. **Zwei Codebasen für ein Feature:** `00_mvp` und `01_more` lösen dasselbe
   Problem, teilen aber keinen Code. Empfehlung: `01_more` ist die Hauptlinie,
   `00_mvp` einfrieren (Referenz, kein weiterer Ausbau) statt Workspace/Shared-Lib
   (gegen Klein-halten) oder Doppelpflege.
3. **`00_mvp` ist ohne Display unbrauchbar:** kein Headless-Modus, keine Tests.
   Minimaler Fix (falls überhaupt): denselben `run_headless`-Schnitt wie in
   `01_more` portieren — oder bewusst nicht, wenn (2) gilt (Freeze + Hinweis).
4. **Versteckte Datenlücken in `00_mvp`:** Dateien mit 0 Byte und Verzeichnisse,
   die nur solche enthalten, werden still verworfen (`size > 0`-Filter);
   `Err`-Fälle (z. B. Permission denied) werden still geschluckt; tiefe Bäume
   laufen über Box-Rekursion (Stack-Risiko); kein Fehlerreport.
5. **`01_more`-Scanner skaliert quadratisch in breiten Verzeichnissen:**
   `find_mut`/`merge_scanned_node` vergleichen pro Batch linear über
   `children` (Name für Name, inkl. `to_string_lossy` pro Ebene). Für typische
   Home-Verzeichnisse ok, für `node_modules`-artige Bäume messbar. Fix ohne neue
   Dep: Kinder pro Verzeichnis in `HashMap` oder Batches erst sammeln, dann
   einmalig mischen.
6. **Headless-Duplikat:** `run_headless` und `gui_main` enthalten je eine eigene
   Scan/Merge-Schleife → in einen Helper (`drain_scan_into(root, rx)`) ziehen.
7. **Headless-UX unvollständig:** kein `--help`, keine Begrenzung (`--depth`,
   `--top`), kein Maschinenformat (`--json`). Alles ohne neue Deps machbar.
8. **`notify 6.1` ist veraltet** (aktuell 8.2.0, s. `deps.md`); Upgrade ist
   breaking und gehört in einen separaten `chore(deps)`-Schritt, nicht in diesen Plan.
9. **Lockfiles fehlen im Repo** → reproduzierbare Builds/CI sind nicht garantiert.

## 4. Empfohlener Scope (kleinste sinnvolle Umsetzung)

- **A. Aufräumen (kein Verhalten):** `rayon` aus `01_more/Cargo.toml` streichen,
  beide Clippy-Warnungen fixen, `fmt` sauber halten, beide `Cargo.lock` einchecken.
- **B. `00_mvp` einfrieren** (entschieden 2026-09-18): Hinweis in `00_mvp`
  (Kommentar + Plan-Dok), keine Feature-Arbeit mehr; Release-Workflow bleibt
  unverändert.
- **C. `01_more` robust(er):** Scan/Merge-Helper vereinheitlichen (Punkt 6),
  `HashMap`-Merge oder Batch-Sammeln (Punkt 5, nur wenn Messung es rechtfertigt).
  Keine neuen Headless-Optionen (entschieden 2026-09-18: Minimalprogramm braucht
  keine `--help`/`--depth`/`--top`/`--json`).
- **D. Tests:** Layout-Invarianten (Flächenerhalt, keine Überlappung, Sortierung),
  Scanner-Kanten (leere Dateien, Symlink, Permission-denied ohne Panic),
  Headless-Flags; `cargo test` + `fmt` + `clippy` grün.
- **E. Container/CI:** `xvfb` + X-Libs (Liste s. §7) in Docker aufnehmen,
  GUI-Smoke unter `xvfb-run` in CI.
- **F. Später, separat:** `notify 6 → 8` (`chore(deps)`), CJK-Breitenmessung statt
  7.2-px-Schätzung.

## 5. Requirements-Abgleich: was fehlt in der Anfrage?

Explizit gefordert war: Review beider Programme, X-Server-Problem beachten,
Tools für sauberen Code, klein halten, Architektur + deepwiki-Doku, `deps.md`
mit Orgs, neueste Versionen + Usage-Examples, Feature-Vorschlag, Plan mit
Dateiliste + Commit-Konvention, Tests + Ausführung, `task.md`, Walkthrough +
Docker-Programme.

Entschieden von wol pumba (2026-09-18):

1. `00_mvp` wird **eingefroren** (Referenz, kein Ausbau); Hauptlinie ist `01_more`.
2. `notify`-Upgrade läuft **später** als separater `chore(deps)`-Schritt.
3. **Keine neuen Headless-Optionen** — Minimalprogramm, `--help`/`--depth`/
   `--top`/`--json` entfallen.

Weiterhin offen (bei Bedarf vor Umsetzung klären):

4. Sollen beide Binaries dauerhaft releast werden (Release-Workflow baut beide)?
5. Akzeptanzkriterien: welche Verzeichnisgrößen/Plattformen (Linux genügt?
   macOS/Windows?), Performance-Ziele für den Scan?

## 6. Kontext-Dateien für einen unabhängigen Agenten

| Datei | Wozu lesen |
|---|---|
| `01_more/src/main.rs` | Hauptlinie (~1300 Zeilen): Tree-Modell, Layout, Scanner, Watcher, Rendering, `truncate_label`, `gui_main`, Headless, Tests |
| `00_mvp/src/main.rs` | Referenz-MVP (285 Zeilen): einfaches Modell, klassisches `squarify`, keine Tests/Headless |
| `01_more/Cargo.toml` | 3 Deps (macroquad, rayon→streichen, notify) + Release-Profil (`panic="abort"`) |
| `01_more/tests/headless_scan.rs` | Integrationstest-Muster (Fixture, `current_exe`, Assertions) |
| `01_more/Cargo.lock`, `00_mvp/Cargo.lock` | Einzucheckende Lockfiles (derzeit beide untracked) |
| `.github/workflows/*.yml` | CI-Matrix (beide Projekte × Linux/Windows), Linux-System-Libs |
| `plan/20260918_01_utf8_crash/{plan,task,deps,walkthrough}.md` | Vorbild für Plan-Stil, Headless-Historie, bekannte Clippy-Ausnahme |
| `plan/20260918_02_review/{task,deps}.md` | Serielle Schritte (diese Aufgabe), Dep-Tabelle mit deepwiki-Schlüsseln |
| `~/.cargo/registry/src/*/macroquad_macro-0.1.8/src/lib.rs` | Beleg: `Window::from_config` = `#[macroquad::main]`-Expansion |
| `https://docs.rs/notify` (8.2.0, verifiziert 2026-09-18) | API-Referenz für den späteren `notify`-Upgrade-Schritt |

Hinweis: deepwiki-MCP war hier nicht verfügbar; Abfragen später der Form
`plops/rs_disk_treemap` bzw. pro Dep `ORG/REPO` aus `deps.md`.

## 7. Neueste-Version-Check (rust-Tools, `cargo search`, docs.rs)

- `macroquad 0.4`: Lock 0.4.16 = neueste `0.4.x` → kein Handlungsbedarf.
- `rayon`: Lock 1.10/1.12-Verbund, aktuell 1.12.0 — irrelevant, Dep wird entfernt.
- `notify`: Lock 6.1.1, aktuell **8.2.0** (breaking: 6→7→8). Kein stilles
  `cargo update` über Major-Grenzen; separater Schritt mit Migrationsbeispiel:

```rust
// notify 8 (docs.rs/notify, Org: notify-rs) — gleiche Grundform wie v6 im Code:
// RecommendedWatcher + RecursiveMode sind geblieben, Channel-Auswahl ist neu.
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
let mut watcher = RecommendedWatcher::new(
    |res| { /* Event verarbeiten */ },
    Config::default(),
)?;
watcher.watch(path, RecursiveMode::Recursive)?;
```

- Keine neue Abhängigkeit wird eingeführt (Vorgabe); daher kein weiteres
  Usage-Example nötig. Test-Tooling bleibt `cargo test/fmt/clippy` aus der
  Toolchain (stabile 1.98.1 im Container), keine Dev-Deps.

## 8. Commit-Konvention (Conventional Commits, ausführliche Bodies)

- Format: `<type>(scope): <kurze Imperativ-Zusammenfassung>` + Leerzeile +
  Body (Was/Warum/Tests) + ggf. `Refs:`.
- Scopes: `01_more`, `00_mvp`, `ci`, `plan`. Types: `chore` (Dep-Entfernung,
  Lockfiles), `refactor` (Helper/Clippy), `feat` (Headless-Flags),
  `test` (neue Tests), `docs` (Plan-Dokumente), `ci` (xvfb/Workflow).
- Ein Commit = ein Type; Sprache Deutsch oder Englisch, konsistent pro Commit;
  Dateinamen + beobachtete Ausgaben (`cargo test`, `cargo clippy`,
  `cargo fmt --check`) nennen. Nie `git add -A` — nur eigene Pfade;
  `target/`-Verzeichnisse nie stagen.
- Beispiel: `chore(01_more): drop unused rayon dependency`

## 9. Docker-Programme (in Container aufnehmen)

Aus CI-Workflow (verifiziert vorhanden) + Befund §2:

- `xvfb` (`xvfb-run`) — GUI-Start und GUI-Smoke-Tests ohne echten X-Server.
- X-/GL-/Sound-Build- und Laufzeit-Libs: `libx11-dev libxi-dev
  libgl1-mesa-dev libasound2-dev` (CI), Laufzeit zusätzlich `libxkbcommon0`
  (belegter `LibraryNotFound`-Fehler ohne sie).
- Sonst nichts: Review, Tests und Verifikation kamen mit vorhandener
  Rust-Toolchain (`cargo build/test/fmt/clippy`) aus.

## 10. Walkthrough

Wird nach vollständiger Implementierung geschrieben
(`plan/20260918_02_review/walkthrough.md`): was wirklich umgesetzt wurde,
testbedingte Abweichungen, Learnings, mögliche Erweiterungen.
