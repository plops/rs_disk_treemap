# Walkthrough: Review-Folgearbeiten `00_mvp` + `01_more` (02_review)

Datum: 2026-09-18 · Stand: implementiert, verifiziert, committed.
Scope-Entscheidungen von wol pumba (s. `plan.md §5`): `00_mvp` einfrieren,
kein `notify`-Upgrade, keine neuen Headless-Optionen.

## Was wirklich implementiert wurde

1. **`chore(01_more): drop unused rayon dependency`** — `rayon 1.10` aus
   `01_more/Cargo.toml` gestrichen (keine einzige Verwendung in `src/`;
   Scan bleibt sequenziell im Bg-Thread). `Cargo.lock` regeneriert
   (rayon-Subbaum mit 52 Zeilen entfernt). Dazu der vorbestehende
   Clippy-Hinweis `unnecessary_sort_by` (01_more:221) via
   `sort_unstable_by_key(Reverse(..))` behoben.
2. **`refactor(01_more): unify scan/merge helper`** — neues
   `merge_scan_batch()` (Watcher-Registrierung über
   `Option<&mut WatcherManager>`, Änderungs-Flag als Rückgabe); beide
   Aufrufstellen (`run_headless`, `gui_main`-Schleife) nutzen es.
   Kein Verhaltenswechsel.
3. **`fix(01_more): survive closed stdout pipe`** — `run_headless` nutzte
   `println!` und panikte bei geschlossener Pipe
   (`... | head`: „failed printing to stdout: Broken pipe“). Neues
   `print_line()` ignoriert gezielt `ErrorKind::BrokenPipe`, alles andere
   panikt weiter. Plus Integrationstest `headless_survives_closed_stdout`.
4. **`test(01_more): layout/scanner coverage`** — 3 Unit-Tests:
   `merge_scan_batch` (true/false), Layout-Invarianten (Sortierung,
   Flächenerhalt < 0,5 %, keine Geschwister-Überlappung), Scanner-Kanten
   (0-Byte-Datei bleibt sichtbar, Symlink wird nicht verfolgt,
   nicht-existenter Pfad ohne Panic). Permission-denied-Fall bewusst
   ausgelassen: Container/CI laufen als root, `chmod 000` wäre dort
   bedeutungslos.
5. **`chore(00_mvp): freeze reference MVP`** — Freeze-Kommentar am
   Dateikopf, `needless_range_loop` → `enumerate()`, `Cargo.lock`
   eingecheckt.
6. **`ci: add linux smoke job`** — neuer `smoke`-Job in
   `.github/workflows/build.yml`: `fmt --check`,
   `clippy --all-targets -- -D warnings`, `cargo test`, Headless-Smoke
   (piped durch `head`, übt nebenbei den EPIPE-Fix) und GUI-Smoke unter
   `xvfb-run` (Erfolg = Timeout-Exit 124, d. h. Event-Loop lief).
7. **`docs(plan): 02_review plan, tasks, deps and walkthrough`**
   (diese Dokumente).

Ergebnis: `cargo test` 9/9 grün (7 Unit + 2 Integration),
`cargo fmt --check` sauber (beide Crates),
`cargo clippy --all-targets` warnungsfrei (beide Crates).

## Testbedingte Änderungen und Messungen

- **EPIPE statt HashMap:** Schritt 4 sah optional einen `HashMap`-Merge vor,
  falls Messungen > 20 % zeigen. Messung: 4000 Dateien (40×100 und 1×4000)
  scannen in 17–77 ms (Debug-Build, inkl. Spawn/Sort/Print). Der Merge-Anteil
  ist vernachlässigbar → kein Umbau, Vorgabe „klein halten“ gewinnt. Dafür
  förderte dieselbe Messreihe den EPIPE-Panic zutage (kleiner, getesteter Fix).
- **Clippy-Neuzugang:** eigenes `print_line` brachte `collapsible_if` ein;
  per Let-Chain (`if let ... && ...`) behoben statt Warnung zu lassen.
- **fmt-Nachlauf:** zwei eigene `cargo fmt`-Läufe nötig (lange
  `assert!`-Zeilen, `use`-Sortierung).

## Verifikation der Szenarien

- `... --headless /tmp/singlewide | head -n 1` → Exit 0, kein „panicked“
  (vorher Exit 101 mit Broken-pipe-Panic).
- Beide GUIs unter `xvfb-run -a` mit Timeout 20 s → Exit 124 (liefen),
  leere stderr. Dafür mussten im Container erst `xvfb`, `libxkbcommon0`,
  `libxi6`, `libx11-6`, `libgl1`, `libasound2t64` installiert werden
  (exakte Paketliste s. unten); ohne sie: `XOpenDisplay`- bzw.
  `LibraryNotFound`-Panics — exakt die im Auftrag erwähnte Docker-Situation.
- CI-Job lokal nicht ausführbar (kein GitHub-Runner), aber YAML per
  `yaml.safe_load` validiert und jeder einzelne Schritt lokal gelaufen.

## Learnings

- Tote Deps findet kein Lint: `rayon` kompilierte still mit, nur
  Quelltext-Suche enttarnt sie. Lockfile-Diff (52 Zeilen) macht die
  Entfernung sichtbar.
- `println!` in CLI-Tools ist ein Pipe-Hazard; ein 10-Zeilen-Helper mit
  `BrokenPipe`-Ausnahme gehört in jeden Headless-Modus.
- `timeout`-Exit-Code 124 eignet sich als billiger GUI-Liveness-Check in CI.
- Repo nutzt `difft` als externen Diff (`diff.external`) — für
  Hunk-Splitting (`git diff --no-ext-diff`) und Reviewer ohne difft
  beachten.

## Mögliche Erweiterungen (bewusst offengelassen)

`notify 6.1 → 8.2.0` als separater `chore(deps)` (breaking, Beispiel in
`plan.md §7`); `HashMap`-Merge erst bei belegtem Bedarf;
CJK-Breitenmessung statt 7.2-px-Schätzung; Release-Workflow-Frage
(beide Binaries dauerhaft?) aus `plan.md §5`.

## Neu in den Docker-Container aufzunehmende Programme

- `xvfb` (enthält `xvfb-run`) — GUI-Start und GUI-Smoke-Tests ohne X-Server.
- Laufzeit-Libs für miniquad/macroquad: `libxkbcommon0`, `libxi6`,
  `libx11-6`, `libgl1`, `libasound2t64` (Ubuntu 26.04-Namen; Build dazu wie
  bisher `libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev`).
  Installiert à la `apt-get install -y xvfb libxkbcommon0 libxi6 libx11-6
  libgl1 libasound2t64`.
- Sonst nichts: alles andere kam aus der vorhandenen Rust-Toolchain
  (1.98.1, `cargo build/test/fmt/clippy`).
