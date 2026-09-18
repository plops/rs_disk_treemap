# task.md — Serielle Implementierungs- und Testschritte (Review-Folgearbeiten)

Jeder Schritt endet mit Validierung; erst bei Grün weiter. Scope-Entscheidungen
(2026-09-18): `00_mvp` einfrieren, kein `notify`-Upgrade, keine neuen
Headless-Optionen (Details s. `plan.md §5`).
`cargo`-Arbeitsverzeichnis je Schritt beachtet (`00_mvp/` bzw. `01_more/`).

## Schritt 1 — Baseline sichern (keine Codeänderung)

1. `cd 01_more && cargo test` → ✅ 4 Unit + 1 Integration grün (Stand 2026-09-18).
2. `cargo fmt --check` in `01_more/` und `00_mvp/` → ✅ sauber.
3. `cargo clippy --all-targets` in beiden → je genau 1 bekannte Warnung
   (`01_more:221 unnecessary_sort_by`, `00_mvp:123 needless_range_loop`).
4. `git status --short` prüfen; `target/`-Verzeichnisse nie stagen.

## Schritt 2 — Aufräumen: tote Dep, Clippy, Lockfiles

1. In `01_more/Cargo.toml` die Zeile `rayon = "1.10"` entfernen
   (unbenutzt, verifiziert per Suche; kein `cargo update` über Majors nötig).
2. Clippy-Fixes: `01_more/src/main.rs:221` →
   `sort_unstable_by_key(|a| std::cmp::Reverse(a.size_bytes))`;
   `00_mvp/src/main.rs:123` → `enumerate()`-Schleife.
3. `cd 01_more && cargo build` (Lockfile ohne rayon neu schreiben lassen),
   ebenso `cd 00_mvp && cargo build`.
4. ✅ `cargo test` (01_more, 5/5), ✅ `cargo fmt --check` (beide),
   ✅ `cargo clippy --all-targets` (beide warnungsfrei).
5. Beide `Cargo.lock` per `git add 00_mvp/Cargo.lock 01_more/Cargo.lock`
   stagen (Binary-Lockfile-Konvention); **nicht** `git add -A`.

## Schritt 3 — `00_mvp` einfrieren (entschieden)

1. Kommentar oben in `00_mvp/src/main.rs`: „Referenz-MVP, eingefroren —
   Hauptlinie ist `01_more`“. Kein Headless-Port, keine Feature-Arbeit.
2. ✅ `cargo build` in `00_mvp/` grün.

## Schritt 4 — `01_more`: Scan/Merge-Helper vereinheitlichen

1. Neuen Helper `drain_scan_into(root: &mut FileNode, rx: &Receiver<ScanEvent>,
   watcher: Option<&mut WatcherManager>)` einführen; beide Aufrufstellen
   (`run_headless`, `gui_main`-Schleife) nutzen ihn, Verhalten unverändert.
2. Optional (nur wenn Messung an breitem Fixture > 20 % zeigt):
   linearer Merge → `HashMap`-Lookup pro Verzeichnis.
3. ✅ `cargo test` 5/5 grün, ✅ `fmt`/`clippy` sauber.

## Schritt 5 — Layout- und Scanner-Tests

1. Layout-Invarianten als Unit-Tests: Flächenerhalt (Summe Kinderflächen ≈
   Elternfläche), keine Überlappung, Größensortierung nach Layout.
2. Scanner-Kanten als Tests: 0-Byte-Datei sichtbar, Symlink nicht verfolgt,
   unverlesbares Verzeichnis → kein Panic (stderr/skip ok).
3. ✅ `cargo test` grün, ✅ `cargo clippy --all-targets` warnungsfrei.

## Schritt 6 — Container/CI: xvfb + GUI-Smoke

1. Docker: `apt-get install -y xvfb libx11-dev libxi-dev libgl1-mesa-dev
   libasound2-dev libxkbcommon0` (Liste aus `plan.md §9`).
2. Smoke: `xvfb-run -a ./target/debug/treemap-disk-analyzer --headless <dir>`
   sowie GUI-Start unter `xvfb-run -a` mit Timeout (Fenster öffnet, kein Panic).
3. CI-Workflow um xvfb-Smoke-Job erweitern (Linux).
4. ✅ Smoke ohne `XOpenDisplay`-/`LibraryNotFound`-Panic.

## Schritt 7 — Dokumente + Commits (Conventional Commits, nur eigene Pfade!)

1. `plan.md`, `task.md` (diese Datei), `deps.md` in
   `plan/20260918_02_review/` finalisieren.
2. Commits in Reihenfolge (Format s. `plan.md §8`):
   1. `chore(01_more): drop unused rayon dependency` (+ Clippy-Fixes, Lockfiles),
   2. `refactor(01_more): unify scan/merge helper` (Schritt 4),
   3. `test(01_more): cover layout invariants and scanner edge cases` (Schritt 5),
   4. `ci: add xvfb gui smoke test` (Schritt 6),
   5. `docs(plan): add 02_review plan, tasks and deps`.
3. Nach allen Commits `walkthrough.md` schreiben (was wirklich umgesetzt,
   testbedingte Abweichungen, Learnings, Erweiterungen).
4. ✅ `git status --short` zeigt nur beabsichtigte Dateien; `git log --oneline`
   zeigt die Commit-Reihe.
