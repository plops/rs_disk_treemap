use macroquad::prelude::*;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashSet, hash_map::DefaultHasher};
use std::ffi::OsStr;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Instant;

// ============================================================================
// 1. Data Structures
// ============================================================================

pub struct FileNode {
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub children: Vec<FileNode>,
    pub is_sorted: bool,
    pub color: Color,

    pub target_rect: Rect,
    pub current_rect: Rect,

    pub growth_pulse: f32,
    pub growth_rate: f64,
    pub last_change_time: Instant,
}

impl FileNode {
    pub fn new(name: String, is_dir: bool, size_bytes: u64) -> Self {
        let color = if is_dir {
            Color::new(0.14, 0.16, 0.20, 1.0)
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

    pub fn find_mut(&mut self, components: &[&OsStr]) -> Option<&mut FileNode> {
        if components.is_empty() {
            return Some(self);
        }
        let target = components[0].to_string_lossy();
        for child in &mut self.children {
            if child.name == target {
                return child.find_mut(&components[1..]);
            }
        }
        None
    }

    pub fn update_file_size(
        &mut self,
        components: &[&OsStr],
        new_size: u64,
        now: Instant,
    ) -> Option<i64> {
        if components.is_empty() {
            let old_size = self.size_bytes;
            let diff = new_size as i64 - old_size as i64;
            if diff != 0 {
                let dt = (now - self.last_change_time).as_secs_f64().max(0.01);
                if diff > 0 {
                    self.growth_pulse = 1.0;
                    self.growth_rate = (diff as f64) / dt;
                }
                self.size_bytes = new_size;
                self.last_change_time = now;
            }
            return Some(diff);
        }

        let target = components[0].to_string_lossy();
        for child in &mut self.children {
            if child.name == target {
                if let Some(diff) = child.update_file_size(&components[1..], new_size, now) {
                    if diff != 0 {
                        self.size_bytes = (self.size_bytes as i64 + diff).max(0) as u64;
                        self.is_sorted = false;
                        if diff > 0 {
                            self.growth_pulse = (self.growth_pulse + 0.3).min(1.0);
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

// ============================================================================
// 2. Sequential Squarified Treemap Layout (Zero-Allocation Loop)
// ============================================================================

pub struct LayoutWorkspace {
    areas: Vec<f64>,
    row: Vec<usize>,
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
    let s2 = (side * side) as f64;
    let sum2 = row_sum * row_sum;

    row.iter()
        .copied()
        .chain(extra)
        .map(|idx| {
            let a = areas[idx];
            if a <= 0.0 {
                0.0
            } else {
                (s2 * a / sum2).max(sum2 / (s2 * a))
            }
        })
        .fold(0.0, f64::max)
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
        for &idx in row {
            children[idx].target_rect = Rect::new(rect.x, rect.y, 0.0, 0.0);
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

        let mut current_y = rect.y;
        for (i, &idx) in row.iter().enumerate() {
            let item_h = if i == row.len() - 1 {
                (rect.y + rect.h) - current_y
            } else {
                ((areas[idx] / row_sum) * rect.h as f64) as f32
            }
            .max(0.0);

            children[idx].target_rect = Rect::new(rect.x, current_y, row_thickness, item_h);
            current_y += item_h;
        }

        rect.x += row_thickness;
        rect.w = (rect.w - row_thickness).max(0.0);
    } else {
        let mut row_thickness = if is_last {
            rect.h
        } else {
            (row_sum / rect.w as f64) as f32
        };
        row_thickness = row_thickness.clamp(0.0, rect.h);

        let mut current_x = rect.x;
        for (i, &idx) in row.iter().enumerate() {
            let item_w = if i == row.len() - 1 {
                (rect.x + rect.w) - current_x
            } else {
                ((areas[idx] / row_sum) * rect.w as f64) as f32
            }
            .max(0.0);

            children[idx].target_rect = Rect::new(current_x, rect.y, item_w, row_thickness);
            current_x += item_w;
        }

        rect.y += row_thickness;
        rect.h = (rect.h - row_thickness).max(0.0);
    }
}

fn squarify_children(node: &mut FileNode, ws: &mut LayoutWorkspace) {
    if node.children.is_empty() || node.size_bytes == 0 {
        return;
    }

    if !node.is_sorted {
        node.children
            .sort_unstable_by_key(|a| std::cmp::Reverse(a.size_bytes));
        node.is_sorted = true;
    }

    let total_bytes: u64 = node.children.iter().map(|c| c.size_bytes).sum();
    if total_bytes == 0 {
        return;
    }

    let total_area = (node.target_rect.w * node.target_rect.h) as f64;

    ws.areas.clear();
    ws.areas.extend(
        node.children
            .iter()
            .map(|c| (c.size_bytes as f64 / total_bytes as f64) * total_area),
    );

    let mut remaining_rect = node.target_rect;
    ws.row.clear();
    let mut row_sum = 0.0;

    for i in 0..node.children.len() {
        if ws.areas[i] <= 0.0 {
            node.children[i].target_rect = Rect::new(remaining_rect.x, remaining_rect.y, 0.0, 0.0);
            continue;
        }

        let side = remaining_rect.w.min(remaining_rect.h);
        let worst_with =
            worst_aspect_ratio(&ws.row, Some(i), &ws.areas, row_sum + ws.areas[i], side);
        let worst_without = worst_aspect_ratio(&ws.row, None, &ws.areas, row_sum, side);

        if ws.row.is_empty() || worst_with <= worst_without {
            ws.row.push(i);
            row_sum += ws.areas[i];
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
                for j in (i + 1)..node.children.len() {
                    node.children[j].target_rect =
                        Rect::new(remaining_rect.x, remaining_rect.y, 0.0, 0.0);
                }
                break;
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
        );
    }
}

fn collapse_descendants(node: &mut FileNode, rect: Rect) {
    for child in &mut node.children {
        child.target_rect = rect;
        collapse_descendants(child, rect);
    }
}

pub fn layout_treemap_sequential(node: &mut FileNode, ws: &mut LayoutWorkspace) {
    if node.children.is_empty() || node.size_bytes == 0 {
        return;
    }

    squarify_children(node, ws);

    for child in &mut node.children {
        if child.is_dir && child.size_bytes > 0 {
            if child.target_rect.w >= 6.0 && child.target_rect.h >= 6.0 {
                child.target_rect = Rect::new(
                    child.target_rect.x + 1.0,
                    child.target_rect.y + 1.0,
                    (child.target_rect.w - 2.0).max(1.0),
                    (child.target_rect.h - 2.0).max(1.0),
                );
                layout_treemap_sequential(child, ws);
            } else {
                let r = child.target_rect;
                collapse_descendants(child, r);
            }
        }
    }
}

fn sum_tree_stats(node: &mut FileNode) -> (u64, u64) {
    if !node.is_dir {
        return (1, node.size_bytes);
    }
    let mut f = 0;
    let mut b = 0;
    for c in &mut node.children {
        let (cf, cb) = sum_tree_stats(c);
        f += cf;
        b += cb;
    }
    if node.size_bytes != b {
        node.size_bytes = b;
        node.is_sorted = false;
    }
    (f, b)
}

// ============================================================================
// 3. Robust Filesystem Scanner
// ============================================================================

pub struct DirectoryBatch {
    pub parent_path: PathBuf,
    pub children: Vec<FileNode>,
}

pub enum ScanEvent {
    Batch(DirectoryBatch),
}

fn is_virtual_or_special_fs(path: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        if path.starts_with("/proc")
            || path.starts_with("/sys")
            || path.starts_with("/dev")
            || path.starts_with("/run")
        {
            return true;
        }
    }
    false
}

fn scan_directory_recursive(dir_path: &Path, tx: &Sender<ScanEvent>, abort: &Arc<AtomicBool>) {
    if abort.load(Ordering::Relaxed) || is_virtual_or_special_fs(dir_path) {
        return;
    }

    let entries = match fs::read_dir(dir_path) {
        Ok(iter) => iter,
        Err(_) => return,
    };

    let mut scanned_children = Vec::new();
    let mut subdirs_to_recurse = Vec::new();

    for entry in entries.flatten() {
        if abort.load(Ordering::Relaxed) {
            return;
        }

        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if file_type.is_symlink() {
            continue;
        }

        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        if file_type.is_dir() {
            if !is_virtual_or_special_fs(&path) {
                scanned_children.push(FileNode::new(name, true, 0));
                subdirs_to_recurse.push(path);
            }
        } else if file_type.is_file() {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if size > (1 << 48) {
                continue;
            }
            scanned_children.push(FileNode::new(name, false, size));
        }
    }

    let _ = tx.send(ScanEvent::Batch(DirectoryBatch {
        parent_path: dir_path.to_path_buf(),
        children: scanned_children,
    }));

    for subdir in subdirs_to_recurse {
        scan_directory_recursive(&subdir, tx, abort);
    }
}

/// Merge one scanned directory batch into the tree.
/// Shared by `run_headless` and the `gui_main` loop so the merge logic
/// exists exactly once. Returns true when the tree changed.
fn merge_scan_batch(
    root_node: &mut FileNode,
    target_dir: &Path,
    batch: &mut DirectoryBatch,
    watcher_mgr: Option<&mut WatcherManager>,
) -> bool {
    if let Some(wm) = watcher_mgr {
        wm.register_dir(&batch.parent_path);
    }
    if let Ok(rel) = batch.parent_path.strip_prefix(target_dir) {
        let components: Vec<&OsStr> = rel.iter().collect();
        if let Some(parent) = root_node.find_mut(&components) {
            for child in batch.children.drain(..) {
                merge_scanned_node(parent, child);
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
            for child in new_child.children.drain(..) {
                merge_scanned_node(existing, child);
            }
        } else if existing.size_bytes != new_child.size_bytes {
            existing.size_bytes = new_child.size_bytes;
        }
    } else {
        new_child.target_rect = parent.target_rect;
        new_child.current_rect = parent.current_rect;
        parent.children.push(new_child);
        parent.is_sorted = false;
    }
}

// ============================================================================
// 4. Platform-Adaptive Live Watcher Manager
// ============================================================================

pub struct WatcherManager {
    pub watcher: Option<RecommendedWatcher>,
    pub rx: Receiver<notify::Result<Event>>,
    pub watched_dirs: HashSet<PathBuf>,
    pub is_native_recursive: bool,
    pub limit_hit: bool,
    pub error_msg: Option<String>,
}

impl WatcherManager {
    pub fn new(root_path: &Path) -> Self {
        let (tx, rx) = channel();

        // macOS (FSEvents) and Windows (ReadDirectoryChangesW) handle recursion
        // efficiently in kernel space with zero watch-descriptor overhead.
        // Linux (Inotify) and BSD (Kqueue) require individual directory handles.
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        let is_native_recursive = true;
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let is_native_recursive = false;

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

        let mut watched_dirs = HashSet::new();
        let mut error_msg = None;
        let mut limit_hit = false;

        if let Some(ref mut watcher) = watcher_opt {
            let mode = if is_native_recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };

            match watcher.watch(root_path, mode) {
                Ok(_) => {
                    watched_dirs.insert(root_path.to_path_buf());
                }
                Err(e) => {
                    eprintln!(
                        "[Watcher] Failed to watch root '{}': {e}",
                        root_path.display()
                    );
                    let err_str = e.to_string();
                    if err_str.contains("os error 28") || err_str.contains("No space left") {
                        limit_hit = true;
                    }
                    error_msg = Some(err_str);
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

    /// Register a newly scanned directory with the watcher pool
    pub fn register_dir(&mut self, path: &Path) {
        if self.is_native_recursive || self.limit_hit || is_virtual_or_special_fs(path) {
            return;
        }

        if self.watched_dirs.contains(path) {
            return;
        }

        if let Some(ref mut watcher) = self.watcher {
            match watcher.watch(path, RecursiveMode::NonRecursive) {
                Ok(_) => {
                    self.watched_dirs.insert(path.to_path_buf());
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("os error 28") || err_str.contains("No space left") {
                        if !self.limit_hit {
                            eprintln!(
                                "\n[Watcher Warning] Linux inotify user watch limit reached!"
                            );
                            eprintln!("Run: sudo sysctl fs.inotify.max_user_watches=524288\n");
                            self.limit_hit = true;
                        }
                    } else {
                        self.error_msg = Some(err_str);
                    }
                }
            }
        }
    }
}

// ============================================================================
// 5. Rendering & Non-Allocating String Hover
// ============================================================================

fn update_animations(node: &mut FileNode, dt: f32, now: Instant) -> bool {
    let mut still_anim = false;

    if (node.target_rect.x - node.current_rect.x).abs() > 0.1
        || (node.target_rect.y - node.current_rect.y).abs() > 0.1
        || (node.target_rect.w - node.current_rect.w).abs() > 0.1
        || (node.target_rect.h - node.current_rect.h).abs() > 0.1
    {
        let t = 1.0 - (-14.0 * dt).exp();
        node.current_rect.x += (node.target_rect.x - node.current_rect.x) * t;
        node.current_rect.y += (node.target_rect.y - node.current_rect.y) * t;
        node.current_rect.w += (node.target_rect.w - node.current_rect.w) * t;
        node.current_rect.h += (node.target_rect.h - node.current_rect.h) * t;
        still_anim = true;
    } else {
        node.current_rect = node.target_rect;
    }

    if node.growth_pulse > 0.0 {
        node.growth_pulse = (node.growth_pulse - dt * 1.5).max(0.0);
        still_anim |= node.growth_pulse > 0.001;
    }

    if node.growth_rate > 0.0 && (now - node.last_change_time).as_secs_f32() > 2.0 {
        node.growth_rate = 0.0;
        still_anim = true;
    }

    for child in &mut node.children {
        still_anim |= update_animations(child, dt, now);
    }
    still_anim
}

fn snap_tree(node: &mut FileNode) {
    node.current_rect = node.target_rect;
    for child in &mut node.children {
        snap_tree(child);
    }
}

fn draw_cushion_rect(rect: Rect, base_color: Color, pulse: f32) {
    let fill_color = if pulse > 0.0 {
        let pulse_color = Color::new(0.3, 1.0, 0.4, 1.0);
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

    if rect.w >= 4.0 && rect.h >= 4.0 {
        let bevel = (rect.w.min(rect.h) * 0.12).clamp(1.0, 5.0);
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            bevel,
            Color::new(1.0, 1.0, 1.0, 0.22),
        );
        draw_rectangle(
            rect.x,
            rect.y,
            bevel,
            rect.h,
            Color::new(1.0, 1.0, 1.0, 0.22),
        );
        draw_rectangle(
            rect.x,
            rect.y + rect.h - bevel,
            rect.w,
            bevel,
            Color::new(0.0, 0.0, 0.0, 0.35),
        );
        draw_rectangle(
            rect.x + rect.w - bevel,
            rect.y,
            bevel,
            rect.h,
            Color::new(0.0, 0.0, 0.0, 0.35),
        );
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            1.0,
            Color::new(0.05, 0.05, 0.08, 0.5),
        );
    }

    if pulse > 0.05 {
        draw_rectangle_lines(
            rect.x - 1.0,
            rect.y - 1.0,
            rect.w + 2.0,
            rect.h + 2.0,
            2.0,
            Color::new(0.4, 1.0, 0.5, pulse * 0.8),
        );
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
    if r.x > view_max.x || (r.x + r.w) < view_min.x || r.y > view_max.y || (r.y + r.h) < view_min.y
    {
        return;
    }

    let screen_x = (r.x + camera_pos.x) * camera_zoom;
    let screen_y = (r.y + camera_pos.y) * camera_zoom;
    let screen_w = r.w * camera_zoom;
    let screen_h = r.h * camera_zoom;

    if screen_w < 1.0 || screen_h < 1.0 {
        return;
    }
    let screen_rect = Rect::new(screen_x, screen_y, screen_w, screen_h);

    if node.children.is_empty() {
        draw_cushion_rect(screen_rect, node.color, node.growth_pulse);
    } else {
        if screen_w < 4.0 || screen_h < 4.0 {
            draw_rectangle(
                screen_x,
                screen_y,
                screen_w,
                screen_h,
                Color::new(0.16, 0.18, 0.23, 1.0),
            );
            return;
        }
        for child in &node.children {
            render_treemap(child, view_min, view_max, camera_pos, camera_zoom);
        }
        draw_rectangle_lines(
            screen_x,
            screen_y,
            screen_w,
            screen_h,
            1.0,
            Color::new(0.0, 0.0, 0.0, 0.5),
        );
    }

    if screen_w > 55.0 && screen_h > 18.0 {
        let label = if node.growth_rate > 1024.0 {
            format!(
                "{} [+{}/s]",
                node.name,
                format_bytes(node.growth_rate as u64)
            )
        } else {
            node.name.clone()
        };

        // Text truncation to avoid overlapping neighboring cells
        let max_chars = ((screen_w - 10.0) / 7.2) as usize;
        let display_label = truncate_label(&label, max_chars);

        draw_text(&display_label, screen_x + 5.0, screen_y + 14.0, 14.0, BLACK);
        draw_text(&display_label, screen_x + 4.0, screen_y + 13.0, 14.0, WHITE);
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
        path_out.push('/');
    }
    path_out.push_str(&node.name);

    for child in &node.children {
        if child.current_rect.contains(pt) {
            return find_hovered_path(child, pt, path_out);
        }
    }
    Some(node)
}

// ============================================================================
// 6. Formatting & Color Generator
// ============================================================================

/// Truncate a label to at most `max_chars` characters (Unicode-safe).
///
/// The previous implementation sliced `&label[..max_chars - 2]` on byte
/// indices and panicked on multi-byte chars (e.g. CJK: end byte index 5
/// inside '还'). This cuts on char boundaries instead and compares the
/// char count, not the byte length.
pub fn truncate_label(label: &str, max_chars: usize) -> String {
    if max_chars <= 3 || label.chars().count() <= max_chars {
        return label.to_string();
    }
    let end = label
        .char_indices()
        .nth(max_chars - 2)
        .map(|(i, _)| i)
        .unwrap_or(label.len());
    format!("{}..", &label[..end])
}

fn get_color_for_filename(name: &str) -> Color {
    let ext = Path::new(name)
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "mp4" | "mkv" | "avi" | "mov" => Color::new(0.72, 0.23, 0.86, 1.0),
        "mp3" | "flac" | "wav" | "ogg" => Color::new(0.94, 0.80, 0.14, 1.0),
        "png" | "jpg" | "jpeg" | "webp" | "gif" => Color::new(0.12, 0.74, 0.90, 1.0),
        "zip" | "rar" | "7z" | "tar" | "gz" => Color::new(0.92, 0.20, 0.20, 1.0),
        "rs" | "cpp" | "c" | "h" | "py" | "js" | "ts" | "txt" | "md" => {
            Color::new(0.16, 0.82, 0.43, 1.0)
        }
        "exe" | "dll" | "so" | "bin" => Color::new(0.24, 0.39, 0.94, 1.0),
        _ => {
            let mut hasher = DefaultHasher::new();
            name.hash(&mut hasher);
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

fn format_bytes(bytes: u64) -> String {
    const SUFFIXES: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut d = bytes as f64;
    let mut i = 0;
    while d >= 1024.0 && i < 4 {
        d /= 1024.0;
        i += 1;
    }
    format!("{:.2} {}", d, SUFFIXES[i])
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Treemap Disk Visualizer".to_string(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        platform: macroquad::miniquad::conf::Platform {
            swap_interval: Some(1),
            ..Default::default()
        },
        ..Default::default()
    }
}

// ============================================================================
// 7. Main Event Loop
// ============================================================================

// ============================================================================
// 8. Headless Mode (servers, containers and CI without an X server)
// ============================================================================

/// First non-flag CLI argument is the scan target.
/// `--headless` / `--scan-only` are flags and never a target path.
pub fn target_from_args(args: &[String]) -> PathBuf {
    args.iter()
        .skip(1)
        .find(|a| *a != "--headless" && *a != "--scan-only")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn resolve_target(args: &[String]) -> (PathBuf, String) {
    let raw_target = target_from_args(args);
    let target_dir = fs::canonicalize(&raw_target).unwrap_or(raw_target);
    let root_name = target_dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| target_dir.to_string_lossy().into_owned());
    (target_dir, root_name)
}

/// True when a GUI window can plausibly be opened.
/// On Linux macroquad/miniquad panics with `XOpenDisplay() failed!` when
/// neither X11 nor Wayland is available, so check before touching the GUI.
pub fn has_graphical_display() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("DISPLAY").is_some_and(|v| !v.is_empty())
            || std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// Scan `target_dir` synchronously (reuses the GUI scanner) and print a
/// size-sorted summary to stdout. Returns the process exit code.
pub fn run_headless(target_dir: &Path, root_name: &str) -> i32 {
    let mut root_node = FileNode::new(root_name.to_string(), true, 0);

    let (scan_tx, scan_rx) = channel();
    let abort = Arc::new(AtomicBool::new(false));
    let target = target_dir.to_path_buf();
    let worker = thread::spawn(move || {
        scan_directory_recursive(&target, &scan_tx, &abort);
    });
    while let Ok(ScanEvent::Batch(mut batch)) = scan_rx.recv() {
        merge_scan_batch(&mut root_node, target_dir, &mut batch, None);
    }
    let _ = worker.join();

    let (file_count, total_bytes) = sum_tree_stats(&mut root_node);
    root_node
        .children
        .sort_unstable_by_key(|a| std::cmp::Reverse(a.size_bytes));

    println!(
        "{}: {} in {} files",
        root_name,
        format_bytes(total_bytes),
        file_count
    );
    for child in &root_node.children {
        let suffix = if child.is_dir { "/" } else { "" };
        println!(
            "{}\t{}{}",
            format_bytes(child.size_bytes),
            child.name,
            suffix
        );
    }
    0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let explicit_headless = args.iter().any(|a| a == "--headless" || a == "--scan-only");

    if explicit_headless || !has_graphical_display() {
        if !explicit_headless {
            eprintln!(
                "No graphical display detected (neither DISPLAY nor WAYLAND_DISPLAY is set); \
                 running in headless scan mode. Pass --headless to silence this note, or run \
                 with an X server (e.g. `xvfb-run ./treemap-disk-analyzer <dir>`) for the GUI."
            );
        }
        let (target_dir, root_name) = resolve_target(&args);
        std::process::exit(run_headless(&target_dir, &root_name));
    }
    // NOTE: macroquad::Window::from_config is what #[macroquad::main] expands
    // to; calling it directly lets the headless check run before miniquad
    // ever calls XOpenDisplay().
    macroquad::Window::from_config(window_conf(), gui_main());
}

async fn gui_main() {
    let (target_dir, root_name) = resolve_target(&std::env::args().collect::<Vec<_>>());

    let mut root_node = FileNode::new(root_name, true, 0);

    let active_scanners = Arc::new(AtomicUsize::new(1));
    let abort_scan = Arc::new(AtomicBool::new(false));

    let (scan_tx, scan_rx) = channel();
    let scan_tx_keep = scan_tx.clone();

    // Spawn primary scanner thread
    {
        let target_dir = target_dir.clone();
        let abort = abort_scan.clone();
        let tx = scan_tx_keep.clone();
        let scanners = active_scanners.clone();
        thread::spawn(move || {
            scan_directory_recursive(&target_dir, &tx, &abort);
            scanners.fetch_sub(1, Ordering::Release);
        });
    }

    // Initialize watcher manager
    let mut watcher_mgr = WatcherManager::new(&target_dir);

    let mut camera_pos = Vec2::ZERO;
    let mut camera_zoom = 1.0f32;
    let mut last_mouse = Vec2::ZERO;

    let mut layout_dirty = true;
    let mut is_animating = true;
    let mut last_layout_time = Instant::now();
    let mut hovered_path_cache = String::with_capacity(256);

    let mut tree_files: u64 = 0;
    let mut tree_bytes: u64 = 0;
    let mut layout_workspace = LayoutWorkspace {
        areas: Vec::new(),
        row: Vec::new(),
    };

    loop {
        let dt = get_frame_time().min(0.05);
        let screen_w = screen_width();
        let screen_h = screen_height();
        let world_canvas = Rect::new(0.0, 44.0, screen_w, (screen_h - 44.0).max(1.0));
        let now = Instant::now();

        // 1. Drain progressive scan batches from worker thread(s)
        while let Ok(ScanEvent::Batch(mut batch)) = scan_rx.try_recv() {
            // (also registers the dir in the watcher pool on
            // non-recursive backends such as Linux inotify)
            if merge_scan_batch(
                &mut root_node,
                &target_dir,
                &mut batch,
                Some(&mut watcher_mgr),
            ) {
                layout_dirty = true;
            }
        }

        // 2. Drain live filesystem events
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

                            let parent_comps = &components[..components.len() - 1];
                            let item_name = components.last().unwrap().to_string_lossy();
                            let is_exists = path.exists();

                            if !is_exists {
                                // Natural removal resolution (covers deletes & moved-out renames)
                                if let Some(parent) = root_node.find_mut(parent_comps)
                                    && let Some(pos) =
                                        parent.children.iter().position(|c| c.name == item_name)
                                {
                                    parent.children.remove(pos);
                                    parent.is_sorted = false;
                                    layout_dirty = true;
                                }
                            } else if let Ok(meta) = fs::metadata(&path) {
                                if meta.is_dir() {
                                    let dir_needs_scan =
                                        if let Some(parent) = root_node.find_mut(parent_comps) {
                                            !parent.children.iter().any(|c| c.name == item_name)
                                        } else {
                                            false
                                        };

                                    if dir_needs_scan {
                                        if let Some(parent) = root_node.find_mut(parent_comps) {
                                            let new_dir =
                                                FileNode::new(item_name.into_owned(), true, 0);
                                            parent.children.push(new_dir);
                                            parent.is_sorted = false;
                                            layout_dirty = true;
                                        }

                                        // Register newly observed directory into watcher pool
                                        watcher_mgr.register_dir(&path);

                                        // Spin off deep background recursion worker
                                        active_scanners.fetch_add(1, Ordering::Release);
                                        let tx = scan_tx_keep.clone();
                                        let abort = abort_scan.clone();
                                        let target = path.clone();
                                        let scanners = active_scanners.clone();
                                        thread::spawn(move || {
                                            scan_directory_recursive(&target, &tx, &abort);
                                            scanners.fetch_sub(1, Ordering::Release);
                                        });
                                    }
                                } else if meta.is_file() {
                                    let new_size = meta.len();
                                    if let Some(diff) =
                                        root_node.update_file_size(&components, new_size, now)
                                    {
                                        if diff != 0 {
                                            layout_dirty = true;
                                        }
                                    } else if let Some(parent) = root_node.find_mut(parent_comps) {
                                        parent.children.push(FileNode::new(
                                            item_name.into_owned(),
                                            false,
                                            new_size,
                                        ));
                                        parent.is_sorted = false;
                                        layout_dirty = true;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[Watcher Event Error] {e}");
                }
            }
        }

        // 3. Debounced Squarified Treemap Execution
        let scanning = active_scanners.load(Ordering::Acquire) > 0;
        if layout_dirty && (!scanning || last_layout_time.elapsed().as_millis() > 100) {
            root_node.target_rect = world_canvas;

            let (f_count, b_count) = sum_tree_stats(&mut root_node);
            tree_files = f_count;
            tree_bytes = b_count;

            layout_treemap_sequential(&mut root_node, &mut layout_workspace);
            layout_dirty = false;
            is_animating = true;
            last_layout_time = Instant::now();
        }

        if scanning {
            snap_tree(&mut root_node);
        } else if is_animating {
            is_animating = update_animations(&mut root_node, dt, now);
        }

        // 4. Input Handling: Pan, Zoom, Reset
        let mouse_screen = Vec2::from(mouse_position());
        if is_mouse_button_down(MouseButton::Left) || is_mouse_button_down(MouseButton::Middle) {
            let delta = (mouse_screen - last_mouse) / camera_zoom;
            camera_pos += delta;
        }
        last_mouse = mouse_screen;

        let wheel = mouse_wheel().1;
        if wheel.abs() > 0.01 {
            let mouse_world_before = (mouse_screen / camera_zoom) - camera_pos;
            if wheel > 0.0 {
                camera_zoom *= 1.15;
            } else {
                camera_zoom /= 1.15;
            }
            camera_zoom = camera_zoom.clamp(0.05, 100.0);
            let mouse_world_after = (mouse_screen / camera_zoom) - camera_pos;
            camera_pos += mouse_world_after - mouse_world_before;
        }

        if is_key_pressed(KeyCode::Space) {
            camera_pos = Vec2::ZERO;
            camera_zoom = 1.0;
        }

        // 5. Draw Treemap Scene
        clear_background(Color::new(0.08, 0.09, 0.12, 1.0));
        let view_min = -camera_pos;
        let view_max = (Vec2::new(screen_w, screen_h) / camera_zoom) - camera_pos;
        render_treemap(&root_node, view_min, view_max, camera_pos, camera_zoom);

        // 6. Non-Overwriting (Zero Allocation) Hover Hit-Detection
        hovered_path_cache.clear();
        let mouse_world = (mouse_screen / camera_zoom) - camera_pos;
        let hovered_node = if mouse_screen.y >= 44.0 {
            find_hovered_path(&root_node, mouse_world, &mut hovered_path_cache)
        } else {
            None
        };

        // 7. Draw HUD
        draw_rectangle(0.0, 0.0, screen_w, 44.0, Color::new(0.06, 0.07, 0.09, 0.95));
        draw_line(
            0.0,
            44.0,
            screen_w,
            44.0,
            1.0,
            Color::new(0.15, 0.17, 0.22, 1.0),
        );

        let watcher_alive = watcher_mgr.watcher.is_some() && watcher_mgr.error_msg.is_none();
        let status_color = if scanning {
            Color::new(0.95, 0.8, 0.1, 1.0)
        } else if watcher_mgr.limit_hit {
            Color::new(1.0, 0.55, 0.0, 1.0)
        } else if watcher_alive {
            Color::new(0.2, 0.85, 0.3, 1.0)
        } else {
            Color::new(0.9, 0.2, 0.2, 1.0)
        };

        let status_text = if scanning {
            "SCANNING...".to_string()
        } else if watcher_mgr.limit_hit {
            format!(
                "WATCHING (INOTIFY LIMIT: {} DIRS)",
                watcher_mgr.watched_dirs.len()
            )
        } else if watcher_alive {
            #[cfg(target_os = "linux")]
            {
                format!(
                    "WATCHING (INOTIFY: {} DIRS)",
                    watcher_mgr.watched_dirs.len()
                )
            }
            #[cfg(target_os = "windows")]
            {
                "WATCHING (ReadDirectoryChangesW)".to_string()
            }
            #[cfg(target_os = "macos")]
            {
                "WATCHING (FSEvents)".to_string()
            }
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            {
                "WATCHING (ACTIVE)".to_string()
            }
        } else {
            format!(
                "WATCHER FAILED: {}",
                watcher_mgr.error_msg.as_deref().unwrap_or("Unknown error")
            )
        };

        draw_text(&status_text, 14.0, 18.0, 15.0, status_color);
        draw_text(format!("Files: {}", tree_files), 310.0, 18.0, 15.0, WHITE);
        draw_text(
            format!("Size: {}", format_bytes(tree_bytes)),
            440.0,
            18.0,
            15.0,
            Color::new(0.2, 0.8, 1.0, 1.0),
        );

        if let Some(node) = hovered_node {
            let mut info = format!("{} ({})", hovered_path_cache, format_bytes(node.size_bytes));
            if node.growth_rate > 1024.0 {
                info.push_str(&format!(
                    "  [Active: {}/s]",
                    format_bytes(node.growth_rate as u64)
                ));
            }
            draw_text(&info, 14.0, 36.0, 14.0, Color::new(0.9, 0.9, 0.9, 1.0));
        } else {
            draw_text(
                "Pan: Left/Middle Drag | Zoom: Wheel | Reset: Space | Target: ",
                14.0,
                36.0,
                14.0,
                GRAY,
            );
            draw_text(
                target_dir.to_string_lossy().as_ref(),
                420.0,
                36.0,
                14.0,
                LIGHTGRAY,
            );
        }

        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_ascii_label() {
        assert_eq!(truncate_label("hello world", 8), "hello ..");
        assert_eq!(truncate_label("hi", 8), "hi");
        assert_eq!(truncate_label("12345678", 8), "12345678");
    }

    #[test]
    fn truncate_cjk_label_never_panics() {
        // Exact crash from the bug report: byte index 5 of "abc还def"
        // sits inside '还' (bytes 3..6), which panicked the old slicer.
        assert_eq!(truncate_label("abc还def", 6), "abc还..");
        assert_eq!(truncate_label("abc还def", 7), "abc还def");
        assert_eq!(truncate_label("还还还还还", 4), "还还..");
    }

    #[test]
    fn truncate_emoji_and_narrow_width() {
        assert_eq!(truncate_label("a🦀b", 3), "a🦀b");
        assert_eq!(truncate_label("a🦀bcd", 4), "a🦀..");
        assert_eq!(truncate_label("anything", 3), "anything");
        assert_eq!(truncate_label("anything", 2), "anything");
    }

    #[test]
    fn target_arg_parsing_skips_flags() {
        let args = vec![
            "treemap-disk-analyzer".to_string(),
            "--headless".to_string(),
            "/tmp".to_string(),
        ];
        assert_eq!(target_from_args(&args), PathBuf::from("/tmp"));
        let args = vec!["treemap-disk-analyzer".to_string()];
        assert_eq!(target_from_args(&args), PathBuf::from("."));
    }
}
