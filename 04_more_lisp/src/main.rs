use macroquad::prelude::*;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashSet, hash_map::DefaultHasher};
use std::ffi::OsStr;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Instant;
fn report_skip(context: &str, path: &Path, err: std::io::Error) {
    eprintln!("skip [{context}]: {}: {err:?}", path.display())
}
struct FileNode {
    name: String,
    is_dir: bool,
    size_bytes: u64,
    children: Vec<FileNode>,
    is_sorted: bool,
    color: Color,
    target_rect: Rect,
    current_rect: Rect,
    growth_pulse: f32,
    growth_rate: f64,
    last_change_time: Instant,
}
impl FileNode {
    fn new(name: String, is_dir: bool, size_bytes: u64) -> Self {
        let color = if is_dir {
            Color::new(0.140, 0.160, 0.20, 1.0)
        } else {
            get_color_for_filename(&name)
        };
        Self {
            name,
            is_dir,
            size_bytes,
            children: Vec::new(),
            is_sorted: false,
            color,
            target_rect: Rect::default(),
            current_rect: Rect::default(),
            growth_pulse: 0.0,
            growth_rate: 0.0,
            last_change_time: Instant::now(),
        }
    }
    fn find_mut(&mut self, components: &[&OsStr]) -> Option<&mut FileNode> {
        let mut current = self;
        for component in components {
            let target = component.to_string_lossy();
            {
                let next = current.children.iter_mut().find(|c| c.name == target)?;
                current = next
            }
        }
        Some(current)
    }
    fn update_file_size(
        &mut self,
        components: &[&OsStr],
        new_size: u64,
        now: Instant,
    ) -> Option<i64> {
        self.update_at(components, 0, new_size, now)
    }
    fn update_at(
        &mut self,
        components: &[&OsStr],
        start: usize,
        new_size: u64,
        now: Instant,
    ) -> Option<i64> {
        if start >= components.len() {
            let old_size = self.size_bytes;
            let diff = new_size as i64 - old_size as i64;
            if diff != 0 {
                let dt = (now - self.last_change_time).as_secs_f64().max(1.00e-2);
                if 0 < diff {
                    self.growth_pulse = 1.0;
                    self.growth_rate = diff as f64 / dt
                }
                self.size_bytes = new_size;
                self.last_change_time = now
            }
            return Some(diff);
        }
        {
            let target = components[start].to_string_lossy();
            for child in &mut self.children {
                if child.name == target {
                    if let Some(diff) = child.update_at(components, start + 1, new_size, now) {
                        if diff != 0 {
                            self.size_bytes = (self.size_bytes as i64 + diff).max(0) as u64;
                            self.is_sorted = false;
                            if 0 < diff {
                                self.growth_pulse = (self.growth_pulse + 0.30).min(1.0)
                            }
                        }
                        return Some(diff);
                    }
                    break;
                }
            }
            None
        }
    }
}
struct LayoutWorkspace {
    areas: Vec<f64>,
    row: Vec<usize>,
}
struct DirectoryBatch {
    parent_path: PathBuf,
    children: Vec<FileNode>,
}
enum ScanEvent {
    Batch(DirectoryBatch),
}
fn worst_aspect_ratio(
    row: &[usize],
    extra: Option<usize>,
    areas: &[f64],
    row_sum: f64,
    side: f32,
) -> f64 {
    if row_sum <= 0.0 || side <= 0.0 {
        return f64::INFINITY;
    }
    {
        let (s2, sum2) = ((side * side) as f64, row_sum * row_sum);
        row.iter()
            .copied()
            .chain(extra)
            .map(|idx: usize| -> f64 {
                let a = areas[idx];
                if a <= 0.0 {
                    0.0
                } else {
                    (s2 * (a / sum2)).max(sum2 / (s2 * a))
                }
            })
            .fold(0.0, f64::max)
    }
}
fn layout_row(
    children: &mut [FileNode],
    row: &[usize],
    areas: &[f64],
    row_sum: f64,
    rect: &mut Rect,
    is_last: bool,
) {
    if row.is_empty() {
        return;
    }
    if rect.w <= 0.0 || rect.h <= 0.0 || row_sum <= 0.0 {
        for k in 0..row.len() {
            children[row[k]].target_rect = Rect::new(rect.x, rect.y, 0.0, 0.0)
        }
        return;
    }
    if rect.w >= rect.h {
        let mut row_thickness = if is_last {
            rect.w
        } else {
            (row_sum / rect.h as f64) as f32
        };
        row_thickness = row_thickness.clamp(0.0, rect.w);
        {
            let mut current_y = rect.y;
            for k in 0..row.len() {
                let idx = row[k];
                let item_h = (if k == row.len() - 1 {
                    (rect.y + rect.h) - current_y
                } else {
                    ((areas[row[k]] / row_sum) * rect.h as f64) as f32
                })
                .max(0.0);
                children[idx].target_rect = Rect::new(rect.x, current_y, row_thickness, item_h);
                current_y += item_h
            }
        }
        rect.x += row_thickness;
        rect.w = (rect.w - row_thickness).max(0.0)
    } else {
        let mut row_thickness = if is_last {
            rect.h
        } else {
            (row_sum / rect.w as f64) as f32
        };
        row_thickness = row_thickness.clamp(0.0, rect.h);
        {
            let mut current_x = rect.x;
            for k in 0..row.len() {
                let idx = row[k];
                let item_w = (if k == row.len() - 1 {
                    (rect.x + rect.w) - current_x
                } else {
                    ((areas[row[k]] / row_sum) * rect.w as f64) as f32
                })
                .max(0.0);
                children[idx].target_rect = Rect::new(current_x, rect.y, item_w, row_thickness);
                current_x += item_w
            }
        }
        rect.y += row_thickness;
        rect.h = (rect.h - row_thickness).max(0.0)
    }
}
fn squarify_children(node: &mut FileNode, ws: &mut LayoutWorkspace) {
    if node.children.is_empty() || node.size_bytes == 0 {
        return;
    }
    if !node.is_sorted {
        node.children
            .sort_unstable_by_key(|a| std::cmp::Reverse(a.size_bytes));
        node.is_sorted = true
    }
    {
        let total_bytes: u64 = node.children.iter().map(|c| c.size_bytes).sum();
        if total_bytes == 0 {
            return;
        }
        {
            let total_area = (node.target_rect.w * node.target_rect.h) as f64;
            ws.areas.clear();
            ws.areas.extend(
                node.children
                    .iter()
                    .map(|c| c.size_bytes as f64 * (total_area / total_bytes as f64)),
            );
            {
                let mut remaining_rect = node.target_rect;
                let mut row_sum = 0.0;
                ws.row.clear();
                for i in 0..node.children.len() {
                    if ws.areas[i] <= 0.0 {
                        node.children[i].target_rect =
                            Rect::new(remaining_rect.x, remaining_rect.y, 0.0, 0.0);
                        continue;
                    } else {
                        {
                            let side = remaining_rect.w.min(remaining_rect.h);
                            let worst_with = worst_aspect_ratio(
                                &ws.row,
                                Some(i),
                                &ws.areas,
                                row_sum + ws.areas[i],
                                side,
                            );
                            let worst_without =
                                worst_aspect_ratio(&ws.row, None, &ws.areas, row_sum, side);
                            if ws.row.is_empty() || worst_with <= worst_without {
                                ws.row.push(i);
                                row_sum += ws.areas[i]
                            } else {
                                layout_row(
                                    &mut node.children,
                                    &ws.row,
                                    &ws.areas,
                                    row_sum,
                                    &mut remaining_rect,
                                    false,
                                );
                                ws.row.clear();
                                ws.row.push(i);
                                row_sum = ws.areas[i];
                                if remaining_rect.w <= 0.0 || remaining_rect.h <= 0.0 {
                                    for j in i + 1..node.children.len() {
                                        node.children[j].target_rect =
                                            Rect::new(remaining_rect.x, remaining_rect.y, 0.0, 0.0)
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
                if !ws.row.is_empty() {
                    layout_row(
                        &mut node.children,
                        &ws.row,
                        &ws.areas,
                        row_sum,
                        &mut remaining_rect,
                        true,
                    )
                }
            }
        }
    }
}
fn sum_tree_stats(node: &mut FileNode) -> (u64, u64) {
    if !node.is_dir {
        return (1, node.size_bytes);
    }
    {
        let mut f: u64 = 0;
        let mut b: u64 = 0;
        for c in &mut node.children {
            let (cf, cb) = sum_tree_stats(c);
            f += cf;
            b += cb
        }
        if node.size_bytes != b {
            node.size_bytes = b;
            node.is_sorted = false
        }
        (f, b)
    }
}
fn collapse_descendants(node: &mut FileNode, rect: Rect) {
    for child in &mut node.children {
        child.target_rect = rect;
        collapse_descendants(child, rect)
    }
}
fn layout_treemap_sequential(node: &mut FileNode, ws: &mut LayoutWorkspace) {
    if node.children.is_empty() || node.size_bytes == 0 {
        return;
    }
    squarify_children(node, ws);
    for child in &mut node.children {
        if child.is_dir && 0 < child.size_bytes {
            if 6.0 <= child.target_rect.w && 6.0 <= child.target_rect.h {
                child.target_rect = Rect::new(
                    child.target_rect.x + 1.0,
                    child.target_rect.y + 1.0,
                    (child.target_rect.w - 2.0).max(1.0),
                    (child.target_rect.h - 2.0).max(1.0),
                );
                layout_treemap_sequential(child, ws)
            } else {
                collapse_descendants(child, child.target_rect)
            }
        }
    }
}
#[cfg(target_os = "linux")]
fn is_virtual_or_special_fs(path: &Path) -> bool {
    path.starts_with("/proc")
        || path.starts_with("/sys")
        || path.starts_with("/dev")
        || path.starts_with("/run")
}
#[cfg(not(target_os = "linux"))]
fn is_virtual_or_special_fs(path: &Path) -> bool {
    false
}
fn scan_directory_recursive(dir_path: &Path, tx: &Sender<ScanEvent>, abort: &Arc<AtomicBool>) {
    if abort.load(Ordering::Relaxed) || is_virtual_or_special_fs(dir_path) {
        return;
    }
    {
        let entries = match fs::read_dir(dir_path) {
            Ok(iter) => iter,
            Err(e) => {
                report_skip("read_dir", dir_path, e);
                return;
            }
        };
        {
            let mut scanned_children = Vec::new();
            let mut subdirs_to_recurse = Vec::new();
            for entry in entries.flatten() {
                if abort.load(Ordering::Relaxed) {
                    return;
                }
                {
                    let file_type = match entry.file_type() {
                        Ok(ft) => ft,
                        Err(e) => {
                            report_skip("file_type", &entry.path(), e);
                            continue;
                        }
                    };
                    if file_type.is_symlink() {
                        continue;
                    }
                    {
                        let path = entry.path();
                        let name = entry.file_name().to_string_lossy().into_owned();
                        if file_type.is_dir() {
                            if !is_virtual_or_special_fs(&path) {
                                scanned_children.push(FileNode::new(name, true, 0));
                                subdirs_to_recurse.push(path)
                            }
                        } else {
                            if file_type.is_file() {
                                let size = match entry.metadata() {
                                    Ok(m) => m.len(),
                                    Err(e) => {
                                        report_skip("metadata", &path, e);
                                        0
                                    }
                                };
                                if !(1 << 48 < size) {
                                    scanned_children.push(FileNode::new(name, false, size))
                                }
                            }
                        }
                    }
                }
            }
            {
                let _ = tx.send(ScanEvent::Batch(DirectoryBatch {
                    parent_path: dir_path.to_path_buf(),
                    children: scanned_children,
                }));
            }
            for subdir in subdirs_to_recurse {
                scan_directory_recursive(&subdir, tx, abort)
            }
        }
    }
}
fn merge_scan_batch(
    root_node: &mut FileNode,
    target_dir: &Path,
    batch: DirectoryBatch,
    watcher_mgr: Option<&mut WatcherManager>,
) -> bool {
    if let Some(wm) = watcher_mgr {
        wm.register_dir(&batch.parent_path)
    }
    if let Ok(rel) = batch.parent_path.strip_prefix(target_dir) {
        let components: Vec<&OsStr> = rel.iter().collect();
        if let Some(parent) = root_node.find_mut(&components) {
            for child in batch.children {
                merge_scanned_node(parent, child)
            }
            return true;
        }
    }
    false
}
fn merge_scanned_node(parent: &mut FileNode, mut new_child: FileNode) {
    if let Some(existing) = parent
        .children
        .iter_mut()
        .find(|c| c.name == new_child.name && c.is_dir == new_child.is_dir)
    {
        if existing.is_dir {
            for child in std::mem::take(&mut new_child.children) {
                merge_scanned_node(existing, child)
            }
        } else {
            if existing.size_bytes != new_child.size_bytes {
                existing.size_bytes = new_child.size_bytes
            }
        }
    } else {
        new_child.target_rect = parent.target_rect;
        new_child.current_rect = parent.current_rect;
        parent.children.push(new_child);
        parent.is_sorted = false
    }
}
struct WatcherManager {
    watcher: Option<RecommendedWatcher>,
    rx: Receiver<notify::Result<Event>>,
    watched_dirs: HashSet<PathBuf>,
    is_native_recursive: bool,
    limit_hit: bool,
    error_msg: Option<String>,
}
impl WatcherManager {
    fn new(root_path: &Path) -> Self {
        let (tx, rx) = (channel::<notify::Result<Event>>)();
        let is_native_recursive = native_recursive();
        let mut watcher_opt = match notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) {
            Ok(w) => Some(w),
            Err(e) => {
                eprintln!("[Watcher] Initialization failed: {e}");
                return Self {
                    watcher: None,
                    rx,
                    watched_dirs: HashSet::new(),
                    is_native_recursive,
                    limit_hit: false,
                    error_msg: Some(format!("Init error: {e}")),
                };
            }
        };
        {
            let mut watched_dirs = HashSet::new();
            let mut error_msg = None;
            let mut limit_hit = false;
            if let Some(watcher) = watcher_opt.as_mut() {
                let mode = if is_native_recursive {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                };
                match watcher.watch(root_path, mode) {
                    Ok(_) => {
                        let _ = watched_dirs.insert(root_path.to_path_buf());
                    }
                    Err(e) => {
                        eprintln!(
                            "[Watcher] Failed to watch root '{}': {e}",
                            root_path.display()
                        );
                        {
                            let err_str = e.to_string();
                            if err_str.contains("os error 28") || err_str.contains("No space left")
                            {
                                limit_hit = true
                            } else {
                                error_msg = Some(err_str)
                            }
                        }
                    }
                }
            }
            Self {
                watcher: watcher_opt,
                rx,
                watched_dirs,
                is_native_recursive,
                limit_hit,
                error_msg,
            }
        }
    }
    fn register_dir(&mut self, path: &Path) {
        if self.is_native_recursive || self.limit_hit || is_virtual_or_special_fs(path) {
            return;
        }
        if self.watched_dirs.contains(path) {
            return;
        }
        if let Some(watcher) = self.watcher.as_mut() {
            match watcher.watch(path, RecursiveMode::NonRecursive) {
                Ok(_) => {
                    let _ = self.watched_dirs.insert(path.to_path_buf());
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("os error 28") || err_str.contains("No space left") {
                        if !self.limit_hit {
                            eprintln!();
                            eprintln!("[Watcher Warning] Linux inotify user watch limit reached!");
                            eprintln!("Run: sudo sysctl fs.inotify.max_user_watches=524288");
                            eprintln!();
                            self.limit_hit = true
                        }
                    } else {
                        self.error_msg = Some(err_str)
                    }
                }
            }
        }
    }
}
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn native_recursive() -> bool {
    true
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn native_recursive() -> bool {
    false
}
fn truncate_label(label: &str, max_chars: usize) -> String {
    if max_chars <= 3 || label.chars().count() <= max_chars {
        return label.to_string();
    }
    {
        let prefix: String = label.chars().take(max_chars - 2).collect();
        format!("{}..", prefix)
    }
}
fn get_color_for_filename(name: &str) -> Color {
    let ext = Path::new(name)
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or("")
        .to_lowercase();
    if ext == "mp4" || ext == "mkv" || ext == "avi" || ext == "mov" {
        Color::new(0.720, 0.230, 0.860, 1.0)
    } else {
        if ext == "mp3" || ext == "flac" || ext == "wav" || ext == "ogg" {
            Color::new(0.940, 0.80, 0.140, 1.0)
        } else {
            if ext == "png" || ext == "jpg" || ext == "jpeg" || ext == "webp" || ext == "gif" {
                Color::new(0.120, 0.740, 0.90, 1.0)
            } else {
                if ext == "zip" || ext == "rar" || ext == "7z" || ext == "tar" || ext == "gz" {
                    Color::new(0.920, 0.20, 0.20, 1.0)
                } else {
                    if ext == "rs"
                        || ext == "cpp"
                        || ext == "c"
                        || ext == "h"
                        || ext == "py"
                        || ext == "js"
                        || ext == "ts"
                        || ext == "txt"
                        || ext == "md"
                    {
                        Color::new(0.160, 0.820, 0.430, 1.0)
                    } else {
                        if ext == "exe" || ext == "dll" || ext == "so" || ext == "bin" {
                            Color::new(0.240, 0.390, 0.940, 1.0)
                        } else {
                            let mut hasher = DefaultHasher::new();
                            name.hash(&mut hasher);
                            {
                                let h = hasher.finish();
                                Color::from_rgba(
                                    ((h >> 16) % 160 + 80) as u8,
                                    ((h >> 8) % 160 + 80) as u8,
                                    (h % 160 + 80) as u8,
                                    255,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}
fn print_line(line: String) {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    if let Err(e) = writeln!(lock, "{line}") {
        match e.kind() {
            io::ErrorKind::BrokenPipe => {}
            _ => {
                panic!("failed printing to stdout: {e}")
            }
        }
    }
}
fn format_bytes(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut d = bytes as f64;
    let mut i: usize = 0;
    while 1024.0 <= d && i < 4 {
        d /= 1024.0;
        i += 1
    }
    format!("{:.2} {}", d, units[i])
}
#[allow(clippy::field_reassign_with_default)]
fn window_conf() -> Conf {
    let mut conf = Conf::default();
    conf.window_title = "Treemap Disk Visualizer".to_string();
    conf.window_width = 1280;
    conf.window_height = 720;
    conf.high_dpi = true;
    conf.platform.swap_interval = Some(1);
    conf
}
fn target_from_args(args: &[String]) -> PathBuf {
    args.iter()
        .skip(1)
        .find(|a| *a != "--headless" && *a != "--scan-only")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
fn resolve_target(args: &[String]) -> (PathBuf, String) {
    let raw_target = target_from_args(args);
    let target_dir = fs::canonicalize(&raw_target).unwrap_or(raw_target);
    {
        let root_name = target_dir
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| target_dir.to_string_lossy().into_owned());
        (target_dir, root_name)
    }
}
fn run_headless(target_dir: &Path, root_name: &str) -> i32 {
    let mut root_node = FileNode::new(root_name.to_string(), true, 0);
    let (scan_tx, scan_rx) = (channel::<ScanEvent>)();
    let abort = Arc::new(AtomicBool::new(false));
    let target = target_dir.to_path_buf();
    eprintln!("scan-start [target.display()]={:?}", target.display());
    {
        let worker = thread::spawn(move || {
            scan_directory_recursive(&target, &scan_tx, &abort);
        });
        while let Ok(ScanEvent::Batch(batch)) = scan_rx.recv() {
            merge_scan_batch(&mut root_node, target_dir, batch, None);
        }
        {
            let _ = worker.join();
        }
        {
            let (file_count, total_bytes) = sum_tree_stats(&mut root_node);
            root_node
                .children
                .sort_unstable_by_key(|a| std::cmp::Reverse(a.size_bytes));
            eprintln!(
                "scan-done [file_count]={:?} [total_bytes]={:?}",
                file_count, total_bytes
            );
            print_line(format!(
                "{}: {} in {} files",
                root_name,
                format_bytes(total_bytes),
                file_count
            ));
            for child in &root_node.children {
                let suffix = if child.is_dir { "/" } else { "" };
                print_line(format!(
                    "{}	{}{}",
                    format_bytes(child.size_bytes),
                    child.name,
                    suffix
                ))
            }
            0
        }
    }
}
#[cfg(target_os = "linux")]
fn has_graphical_display() -> bool {
    std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty())
        || std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
}
#[cfg(not(target_os = "linux"))]
fn has_graphical_display() -> bool {
    true
}
fn update_animations(node: &mut FileNode, dt: f32, now: Instant) -> bool {
    let mut still_anim = false;
    if 0.10 < (node.target_rect.x - node.current_rect.x).abs()
        || 0.10 < (node.target_rect.y - node.current_rect.y).abs()
        || 0.10 < (node.target_rect.w - node.current_rect.w).abs()
        || 0.10 < (node.target_rect.h - node.current_rect.h).abs()
    {
        let tt = 1.0 - ((-14.0) * dt).exp();
        node.current_rect.x = node.current_rect.x + (node.target_rect.x - node.current_rect.x) * tt;
        node.current_rect.y = node.current_rect.y + (node.target_rect.y - node.current_rect.y) * tt;
        node.current_rect.w = node.current_rect.w + (node.target_rect.w - node.current_rect.w) * tt;
        node.current_rect.h = node.current_rect.h + (node.target_rect.h - node.current_rect.h) * tt;
        still_anim = true
    } else {
        node.current_rect = node.target_rect
    }
    if 0.0 < node.growth_pulse {
        node.growth_pulse = (node.growth_pulse - dt * 1.50).max(0.0);
        still_anim = still_anim || 1.00e-3 < node.growth_pulse
    }
    if 0.0 < node.growth_rate && 2.0 < (now - node.last_change_time).as_secs_f32() {
        node.growth_rate = 0.0;
        still_anim = true
    }
    for child in &mut node.children {
        still_anim = update_animations(child, dt, now) || still_anim
    }
    still_anim
}
fn snap_tree(node: &mut FileNode) {
    node.current_rect = node.target_rect;
    for child in &mut node.children {
        snap_tree(child)
    }
}
fn draw_cushion_rect(rect: Rect, base_color: Color, pulse: f32) {
    let fill_color = if 0.0 < pulse {
        let pulse_color = Color::new(0.30, 1.0, 0.40, 1.0);
        Color::new(
            base_color.r * (1.0 - pulse) + pulse_color.r * pulse,
            base_color.g * (1.0 - pulse) + pulse_color.g * pulse,
            base_color.b * (1.0 - pulse) + pulse_color.b * pulse,
            1.0,
        )
    } else {
        base_color
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill_color);
    if 4.0 <= rect.w && 4.0 <= rect.h {
        let bevel = (rect.w.min(rect.h) * 0.120).clamp(1.0, 5.0);
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            bevel,
            Color::new(1.0, 1.0, 1.0, 0.220),
        );
        draw_rectangle(
            rect.x,
            rect.y,
            bevel,
            rect.h,
            Color::new(1.0, 1.0, 1.0, 0.220),
        );
        draw_rectangle(
            rect.x,
            rect.y + (rect.h - bevel),
            rect.w,
            bevel,
            Color::new(0.0, 0.0, 0.0, 0.350),
        );
        draw_rectangle(
            rect.x + (rect.w - bevel),
            rect.y,
            bevel,
            rect.h,
            Color::new(0.0, 0.0, 0.0, 0.350),
        );
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            1.0,
            Color::new(5.00e-2, 5.00e-2, 8.00e-2, 0.50),
        )
    }
    if 5.00e-2 < pulse {
        draw_rectangle_lines(
            rect.x - 1.0,
            rect.y - 1.0,
            rect.w + 2.0,
            rect.h + 2.0,
            2.0,
            Color::new(0.40, 1.0, 0.50, pulse * 0.80),
        )
    }
}
fn render_treemap(
    node: &FileNode,
    view_min: Vec2,
    view_max: Vec2,
    camera_pos: Vec2,
    camera_zoom: f32,
) {
    let r = node.current_rect;
    if view_max.x < r.x || r.x + r.w < view_min.x || view_max.y < r.y || r.y + r.h < view_min.y {
        return;
    }
    {
        let screen_x = (r.x + camera_pos.x) * camera_zoom;
        let screen_y = (r.y + camera_pos.y) * camera_zoom;
        let screen_w = r.w * camera_zoom;
        let screen_h = r.h * camera_zoom;
        if screen_w < 1.0 || screen_h < 1.0 {
            return;
        }
        {
            let screen_rect = Rect::new(screen_x, screen_y, screen_w, screen_h);
            if node.children.is_empty() {
                draw_cushion_rect(screen_rect, node.color, node.growth_pulse)
            } else {
                if screen_w < 4.0 || screen_h < 4.0 {
                    draw_rectangle(
                        screen_x,
                        screen_y,
                        screen_w,
                        screen_h,
                        Color::new(0.160, 0.180, 0.230, 1.0),
                    );
                    return;
                }
                for child in &node.children {
                    render_treemap(child, view_min, view_max, camera_pos, camera_zoom)
                }
                draw_rectangle_lines(
                    screen_x,
                    screen_y,
                    screen_w,
                    screen_h,
                    1.0,
                    Color::new(0.0, 0.0, 0.0, 0.50),
                )
            }
            if 55.0 < screen_w && 18.0 < screen_h {
                let label = if 1024.0 < node.growth_rate {
                    format!(
                        "{} [+{}/s]",
                        node.name,
                        format_bytes(node.growth_rate as u64)
                    )
                } else {
                    node.name.clone()
                };
                {
                    let max_chars = ((screen_w - 10.0) / 7.20) as usize;
                    let display_label = truncate_label(&label, max_chars);
                    draw_text(&display_label, screen_x + 5.0, screen_y + 14.0, 14.0, BLACK);
                    draw_text(&display_label, screen_x + 4.0, screen_y + 13.0, 14.0, WHITE);
                }
            }
        }
    }
}
fn find_hovered_path<'a>(
    node: &'a FileNode,
    pt: Vec2,
    path_out: &mut String,
) -> Option<&'a FileNode> {
    if !node.current_rect.contains(pt) {
        return None;
    }
    if !path_out.is_empty() {
        path_out.push('/')
    }
    path_out.push_str(&node.name);
    for child in &node.children {
        if child.current_rect.contains(pt) {
            return find_hovered_path(child, pt, path_out);
        }
    }
    Some(node)
}
#[cfg(target_os = "linux")]
fn backend_label(mgr: &WatcherManager) -> String {
    format!("WATCHING (INOTIFY: {} DIRS)", mgr.watched_dirs.len())
}
#[cfg(target_os = "windows")]
fn backend_label(mgr: &WatcherManager) -> String {
    "WATCHING (ReadDirectoryChangesW)".to_string()
}
#[cfg(target_os = "macos")]
fn backend_label(mgr: &WatcherManager) -> String {
    "WATCHING (FSEvents)".to_string()
}
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn backend_label(mgr: &WatcherManager) -> String {
    "WATCHING (ACTIVE)".to_string()
}
async fn gui_main() {
    let collected: Vec<String> = std::env::args().collect();
    {
        let (target_dir, root_name) = resolve_target(&collected);
        {
            let mut root_node = FileNode::new(root_name, true, 0);
            let active_scanners = Arc::new(AtomicUsize::new(1));
            let abort_scan = Arc::new(AtomicBool::new(false));
            let (scan_tx, scan_rx) = channel();
            {
                let scan_tx_keep = scan_tx.clone();
                {
                    let spawn_target = target_dir.clone();
                    let spawn_abort = abort_scan.clone();
                    let spawn_tx = scan_tx_keep.clone();
                    let spawn_scanners = active_scanners.clone();
                    thread::spawn(move || {
                        scan_directory_recursive(&spawn_target, &spawn_tx, &spawn_abort);
                        spawn_scanners.fetch_sub(1, std::sync::atomic::Ordering::Release);
                    });
                }
                {
                    let mut watcher_mgr = WatcherManager::new(&target_dir);
                    let mut camera_pos = Vec2::ZERO;
                    let mut camera_zoom = 1.0;
                    let mut last_mouse = Vec2::ZERO;
                    let mut layout_dirty = true;
                    let mut is_animating = true;
                    let mut last_layout_time = Instant::now();
                    let mut hovered_path_cache = String::with_capacity(256);
                    let mut tree_files = 0;
                    let mut tree_bytes = 0;
                    let mut layout_workspace = LayoutWorkspace {
                        areas: Vec::new(),
                        row: Vec::new(),
                    };
                    loop {
                        let dt = get_frame_time().min(5.00e-2);
                        let screen_w = screen_width();
                        let screen_h = screen_height();
                        let world_canvas =
                            Rect::new(0.0, 44.0, screen_w, (screen_h - 44.0).max(1.0));
                        let now = Instant::now();
                        while let Ok(ScanEvent::Batch(batch)) = scan_rx.try_recv() {
                            if merge_scan_batch(
                                &mut root_node,
                                &target_dir,
                                batch,
                                Some(&mut watcher_mgr),
                            ) {
                                layout_dirty = true
                            }
                        }
                        while let Ok(res) = watcher_mgr.rx.try_recv() {
                            match res {
                                Ok(event) => {
                                    if matches!(event.kind, EventKind::Access(_)) {
                                        continue;
                                    }
                                    for path in event.paths {
                                        if is_virtual_or_special_fs(&path) {
                                            continue;
                                        }
                                        if let Ok(rel) = path.strip_prefix(&target_dir) {
                                            let components: Vec<&OsStr> = rel.iter().collect();
                                            if components.is_empty() {
                                                continue;
                                            }
                                            {
                                                let parent_comps =
                                                    components.split_at(components.len() - 1).0;
                                                let item_name = components[components.len() - 1]
                                                    .to_string_lossy();
                                                let is_exists = path.exists();
                                                if !is_exists {
                                                    #[allow(clippy::collapsible_if)]
                                                    if let Some(parent) =
                                                        root_node.find_mut(parent_comps)
                                                    {
                                                        if let Some(pos) = parent
                                                            .children
                                                            .iter()
                                                            .position(|c| c.name == item_name)
                                                        {
                                                            parent.children.remove(pos);
                                                            parent.is_sorted = false;
                                                            layout_dirty = true
                                                        }
                                                    }
                                                } else {
                                                    if let Ok(meta) = fs::metadata(&path) {
                                                        if meta.is_dir() {
                                                            let dir_needs_scan =
                                                                if let Some(parent) =
                                                                    root_node.find_mut(parent_comps)
                                                                {
                                                                    !parent.children.iter().any(
                                                                        |c| c.name == item_name,
                                                                    )
                                                                } else {
                                                                    false
                                                                };
                                                            if dir_needs_scan {
                                                                if let Some(parent) =
                                                                    root_node.find_mut(parent_comps)
                                                                {
                                                                    parent.children.push(
                                                                        FileNode::new(
                                                                            item_name.into_owned(),
                                                                            true,
                                                                            0,
                                                                        ),
                                                                    );
                                                                    parent.is_sorted = false;
                                                                    layout_dirty = true
                                                                }
                                                                watcher_mgr.register_dir(&path);
                                                                active_scanners.fetch_add(
                                                                    1,
                                                                    Ordering::Release,
                                                                );
                                                                {
                                                                    let deep_tx =
                                                                        scan_tx_keep.clone();
                                                                    let deep_abort =
                                                                        abort_scan.clone();
                                                                    let deep_target = path.clone();
                                                                    let deep_scanners =
                                                                        active_scanners.clone();
                                                                    thread::spawn(move || {
                                                                        scan_directory_recursive(
                                                                            &deep_target,
                                                                            &deep_tx,
                                                                            &deep_abort,
                                                                        );
                                                                        deep_scanners.fetch_sub(1, std::sync::atomic::Ordering::Release);
                                                                    });
                                                                }
                                                            }
                                                        } else {
                                                            if meta.is_file() {
                                                                let new_size = meta.len();
                                                                if let Some(diff) = root_node
                                                                    .update_file_size(
                                                                        &components,
                                                                        new_size,
                                                                        now,
                                                                    )
                                                                {
                                                                    if diff != 0 {
                                                                        layout_dirty = true
                                                                    }
                                                                } else {
                                                                    if let Some(parent) = root_node
                                                                        .find_mut(parent_comps)
                                                                    {
                                                                        parent.children.push(
                                                                            FileNode::new(
                                                                                item_name
                                                                                    .into_owned(),
                                                                                false,
                                                                                new_size,
                                                                            ),
                                                                        );
                                                                        parent.is_sorted = false;
                                                                        layout_dirty = true
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[Watcher Event Error] {e}")
                                }
                            }
                        }
                        {
                            let scanning = 0 < active_scanners.load(Ordering::Acquire);
                            if layout_dirty
                                && (!scanning || 100 < last_layout_time.elapsed().as_millis())
                            {
                                root_node.target_rect = world_canvas;
                                {
                                    let (f_count, b_count) = sum_tree_stats(&mut root_node);
                                    tree_files = f_count;
                                    tree_bytes = b_count
                                }
                                layout_treemap_sequential(&mut root_node, &mut layout_workspace);
                                layout_dirty = false;
                                is_animating = true;
                                last_layout_time = Instant::now();
                                eprintln!(
                                    "layout [tree_files]={:?} [tree_bytes]={:?}",
                                    tree_files, tree_bytes
                                )
                            }
                            if scanning {
                                snap_tree(&mut root_node)
                            } else {
                                if is_animating {
                                    is_animating = update_animations(&mut root_node, dt, now)
                                }
                            }
                            {
                                let mouse_screen = Vec2::from(mouse_position());
                                if is_mouse_button_down(MouseButton::Left)
                                    || is_mouse_button_down(MouseButton::Middle)
                                {
                                    let delta = (mouse_screen - last_mouse) / camera_zoom;
                                    camera_pos += delta
                                }
                                last_mouse = mouse_screen;
                                {
                                    let wheel = mouse_wheel().1;
                                    if 1.00e-2 < wheel.abs() {
                                        let mouse_world_before =
                                            mouse_screen / camera_zoom - camera_pos;
                                        if 0.0 < wheel {
                                            camera_zoom *= 1.15;
                                        } else {
                                            camera_zoom /= 1.15;
                                        }
                                        camera_zoom = camera_zoom.clamp(5.00e-2, 1.00e+2);
                                        {
                                            let mouse_world_after =
                                                mouse_screen / camera_zoom - camera_pos;
                                            camera_pos += mouse_world_after - mouse_world_before
                                        }
                                    }
                                    if is_key_pressed(KeyCode::Space) {
                                        camera_pos = Vec2::ZERO;
                                        camera_zoom = 1.0
                                    }
                                    {
                                        clear_background(Color::new(8.00e-2, 9.00e-2, 0.120, 1.0));
                                        {
                                            let view_min = -camera_pos;
                                            let view_max = Vec2::new(screen_w, screen_h)
                                                / camera_zoom
                                                - camera_pos;
                                            render_treemap(
                                                &root_node,
                                                view_min,
                                                view_max,
                                                camera_pos,
                                                camera_zoom,
                                            );
                                            hovered_path_cache.clear();
                                            {
                                                let mouse_world =
                                                    mouse_screen / camera_zoom - camera_pos;
                                                let hovered_node = if mouse_screen.y >= 44.0 {
                                                    find_hovered_path(
                                                        &root_node,
                                                        mouse_world,
                                                        &mut hovered_path_cache,
                                                    )
                                                } else {
                                                    None
                                                };
                                                {
                                                    draw_rectangle(
                                                        0.0,
                                                        0.0,
                                                        screen_w,
                                                        44.0,
                                                        Color::new(
                                                            6.00e-2, 7.00e-2, 9.00e-2, 0.950,
                                                        ),
                                                    );
                                                    draw_line(
                                                        0.0,
                                                        44.0,
                                                        screen_w,
                                                        44.0,
                                                        1.0,
                                                        Color::new(0.150, 0.170, 0.220, 1.0),
                                                    );
                                                    {
                                                        let watcher_alive =
                                                            watcher_mgr.watcher.is_some()
                                                                && watcher_mgr.error_msg.is_none();
                                                        {
                                                            let status_color = if scanning {
                                                                Color::new(0.950, 0.80, 0.10, 1.0)
                                                            } else {
                                                                if watcher_mgr.limit_hit {
                                                                    Color::new(1.0, 0.550, 0.0, 1.0)
                                                                } else {
                                                                    if watcher_alive {
                                                                        Color::new(
                                                                            0.20, 0.850, 0.30, 1.0,
                                                                        )
                                                                    } else {
                                                                        Color::new(
                                                                            0.90, 0.20, 0.20, 1.0,
                                                                        )
                                                                    }
                                                                }
                                                            };
                                                            {
                                                                let status_text = if scanning {
                                                                    "SCANNING...".to_string()
                                                                } else {
                                                                    if watcher_mgr.limit_hit {
                                                                        format!(
                                                                            "WATCHING (INOTIFY LIMIT: {} DIRS)",
                                                                            watcher_mgr
                                                                                .watched_dirs
                                                                                .len()
                                                                        )
                                                                    } else {
                                                                        if watcher_alive {
                                                                            backend_label(
                                                                                &watcher_mgr,
                                                                            )
                                                                        } else {
                                                                            format!("WATCHER FAILED: {}", watcher_mgr.error_msg.as_deref().unwrap_or("Unknown error"))
                                                                        }
                                                                    }
                                                                };
                                                                draw_text(
                                                                    &status_text,
                                                                    14.0,
                                                                    18.0,
                                                                    15.0,
                                                                    status_color,
                                                                );
                                                                draw_text(
                                                                    format!(
                                                                        "Files: {}",
                                                                        tree_files
                                                                    ),
                                                                    3.10e+2,
                                                                    18.0,
                                                                    15.0,
                                                                    WHITE,
                                                                );
                                                                draw_text(
                                                                    format!(
                                                                        "Size: {}",
                                                                        format_bytes(tree_bytes)
                                                                    ),
                                                                    4.40e+2,
                                                                    18.0,
                                                                    15.0,
                                                                    Color::new(
                                                                        0.20, 0.80, 1.0, 1.0,
                                                                    ),
                                                                );
                                                                if let Some(node) = hovered_node {
                                                                    let mut info = format!(
                                                                        "{} ({})",
                                                                        hovered_path_cache,
                                                                        format_bytes(
                                                                            node.size_bytes
                                                                        )
                                                                    );
                                                                    if 1024.0 < node.growth_rate {
                                                                        info.push_str(&format!(
                                                                            "  [Active: {}/s]",
                                                                            format_bytes(
                                                                                node.growth_rate
                                                                                    as u64
                                                                            )
                                                                        ))
                                                                    }
                                                                    draw_text(
                                                                        &info,
                                                                        14.0,
                                                                        36.0,
                                                                        14.0,
                                                                        Color::new(
                                                                            0.90, 0.90, 0.90, 1.0,
                                                                        ),
                                                                    );
                                                                } else {
                                                                    draw_text(
                                                                        "Pan: Left/Middle Drag | Zoom: Wheel | Reset: Space | Target: ",
                                                                        14.0,
                                                                        36.0,
                                                                        14.0,
                                                                        GRAY,
                                                                    );
                                                                    draw_text(
                                                                        target_dir
                                                                            .to_string_lossy()
                                                                            .as_ref(),
                                                                        4.20e+2,
                                                                        36.0,
                                                                        14.0,
                                                                        LIGHTGRAY,
                                                                    );
                                                                }
                                                                next_frame().await;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn truncate_ascii_label() {
    assert_eq!(truncate_label("hello world", 8), "hello ..");
    assert_eq!(truncate_label("hi", 8), "hi");
    assert_eq!(truncate_label("12345678", 8), "12345678")
}
#[test]
fn truncate_cjk_label_never_panics() {
    assert_eq!(truncate_label("abc还def", 6), "abc还..");
    assert_eq!(truncate_label("abc还def", 7), "abc还def");
    assert_eq!(truncate_label("还还还还还", 4), "还还..")
}
#[test]
fn truncate_emoji_and_narrow_width() {
    assert_eq!(truncate_label("a🦀b", 3), "a🦀b");
    assert_eq!(truncate_label("a🦀bcd", 4), "a🦀..");
    assert_eq!(truncate_label("anything", 3), "anything");
    assert_eq!(truncate_label("anything", 2), "anything")
}
#[test]
fn target_arg_parsing_skips_flags() {
    {
        let args = vec![
            "treemap-more-lisp".to_string(),
            "--headless".to_string(),
            "/tmp".to_string(),
        ];
        assert_eq!(target_from_args(&args), PathBuf::from("/tmp"))
    }
    {
        let bare = vec!["treemap-more-lisp".to_string()];
        assert_eq!(target_from_args(&bare), PathBuf::from("."))
    }
}
#[test]
fn merge_scan_batch_reports_whether_tree_changed() {
    let target = PathBuf::from("/tmp/treemap-merge-probe");
    let mut root = FileNode::new("root".to_string(), true, 0);
    {
        let batch = DirectoryBatch {
            parent_path: target.clone(),
            children: vec![FileNode::new("a.txt".to_string(), false, 10)],
        };
        assert!(merge_scan_batch(&mut root, &target, batch, None));
        assert_eq!(root.children.len(), 1)
    }
    {
        let orphan = DirectoryBatch {
            parent_path: target.join("nope"),
            children: vec![FileNode::new("b.txt".to_string(), false, 5)],
        };
        assert!(!merge_scan_batch(&mut root, &target, orphan, None));
        assert_eq!(root.children.len(), 1)
    }
}
#[cfg(test)]
fn layout_test_tree() -> FileNode {
    let mut root = FileNode::new("root".to_string(), true, 0);
    root.target_rect = Rect::new(0.0, 0.0, 1.00e+3, 1.00e+3);
    {
        let mut big = FileNode::new("big".to_string(), true, 0);
        big.children = vec![
            FileNode::new("b1".to_string(), false, 400),
            FileNode::new("b2".to_string(), false, 200),
        ];
        root.children = vec![
            big,
            FileNode::new("mid".to_string(), false, 300),
            FileNode::new("small".to_string(), false, 100),
        ]
    }
    sum_tree_stats(&mut root);
    root
}
#[test]
fn layout_conserves_area_sorts_and_avoids_overlap() {
    let mut root = layout_test_tree();
    let mut ws = LayoutWorkspace {
        areas: Vec::new(),
        row: Vec::new(),
    };
    layout_treemap_sequential(&mut root, &mut ws);
    {
        let sizes: Vec<u64> = root.children.iter().map(|c| c.size_bytes).collect();
        assert_eq!(sizes, vec![600, 300, 100])
    }
    {
        let parent_area = root.target_rect.w * root.target_rect.h;
        let children_area: f32 = root
            .children
            .iter()
            .map(|c| c.target_rect.w * c.target_rect.h)
            .sum();
        assert!((children_area - parent_area).abs() / parent_area < 5.00e-3)
    }
    for ai in 0..root.children.len() {
        for bi in ai + 1..root.children.len() {
            let ra = root.children[ai].target_rect;
            let rb = root.children[bi].target_rect;
            assert!(
                !(ra.x < rb.x + rb.w
                    && rb.x < ra.x + ra.w
                    && ra.y < rb.y + rb.h
                    && rb.y < ra.y + ra.h)
            )
        }
    }
}
#[test]
fn update_animations_advances_all_siblings_in_one_pass() {
    let mut root = FileNode::new("root".to_string(), true, 0);
    root.children = vec![
        FileNode::new("c1".to_string(), false, 400),
        FileNode::new("c2".to_string(), false, 200),
    ];
    root.children[0].target_rect = Rect::new(2.00e+2, 0.0, 50.0, 50.0);
    root.children[0].current_rect = Rect::new(0.0, 0.0, 50.0, 50.0);
    root.children[1].target_rect = Rect::new(0.0, 2.00e+2, 50.0, 50.0);
    root.children[1].current_rect = Rect::new(0.0, 0.0, 50.0, 50.0);
    {
        let still = update_animations(&mut root, 1.60e-2, Instant::now());
        assert!(still);
        assert!(0.0 < root.children[0].current_rect.x);
        assert!(0.0 < root.children[1].current_rect.y)
    }
}
fn main() {
    {
        let args: Vec<String> = std::env::args().collect();
        {
            let explicit_headless = args.iter().any(|a| a == "--headless" || a == "--scan-only");
            if explicit_headless || !has_graphical_display() {
                if !explicit_headless {
                    eprintln!(
                        "No graphical display detected (neither DISPLAY nor WAYLAND_DISPLAY is set); running in headless scan mode. Pass --headless to silence this note, or run with an X server (e.g. `xvfb-run ./treemap-disk-analyzer <dir>`) for the GUI."
                    )
                }
                {
                    let (target_dir, root_name) = resolve_target(&args);
                    std::process::exit(run_headless(&target_dir, &root_name))
                }
            }
        }
    }
    macroquad::Window::from_config(window_conf(), gui_main())
}
