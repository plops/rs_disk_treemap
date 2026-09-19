;; gui.lisp --- emitters for async gui_main (setup, drains, loop).
;; Split into <=60-line emitters spliced by emit-gui-main.
;; Reformulations: split_at instead of [..len-1]; nested if-let for
;; let-chains; backend_label helpers instead of cfg-blocks in branches.
(in-package :cl-rust-generator)
(defun emit-backend-label ()
  (list
    `(attr "cfg(target_os = \"linux\")"
       (defun backend_label (&mgr)
         (declare (type WatcherManager &mgr) (values String))
         (format! (string "WATCHING (INOTIFY: {} DIRS)")
                  (dot (dot mgr watched_dirs) (len))
                  )
         )
       )
    `(attr "cfg(target_os = \"windows\")"
       (defun backend_label (&mgr)
         (declare (type WatcherManager &mgr) (values String))
         (dot (string "WATCHING (ReadDirectoryChangesW)") (to_string))
         )
       )
    `(attr "cfg(target_os = \"macos\")"
       (defun backend_label (&mgr)
         (declare (type WatcherManager &mgr) (values String))
         (dot (string "WATCHING (FSEvents)") (to_string))
         )
       )
    `(attr "cfg(not(any(target_os = \"linux\", target_os = \"windows\", target_os = \"macos\")))"
       (defun backend_label (&mgr)
         (declare (type WatcherManager &mgr) (values String))
         (dot (string "WATCHING (ACTIVE)") (to_string))
         )
       )
    )
  )
(defun emit-gui-setup ()
  (list
    `(let ((collected (dot (std--env--args) (collect))))
       (declare (type "Vec<String>" collected))
       )
    `(let (((tuple target_dir root_name) (resolve_target (ref collected)))))
    `(let ((root_node (FileNode--new root_name true 0))
           (active_scanners (Arc--new (AtomicUsize--new 1)))
           (abort_scan (Arc--new (AtomicBool--new false)))
           ((tuple scan_tx scan_rx) (channel))
           )
       (declare (mutable root_node))
       )
    `(let ((scan_tx_keep (dot scan_tx (clone)))))
    `(let ((spawn_target (dot target_dir (clone)))
           (spawn_abort (dot abort_scan (clone)))
           (spawn_tx (dot scan_tx_keep (clone)))
           (spawn_scanners (dot active_scanners (clone)))
           )
       ;; HATCH: move closure as a whole spawn statement (as in 03).
       "thread::spawn(move || { scan_directory_recursive(&spawn_target, &spawn_tx, &spawn_abort); spawn_scanners.fetch_sub(1, std::sync::atomic::Ordering::Release); });"
       )
    `(let ((watcher_mgr (WatcherManager--new (ref target_dir)))
           (camera_pos Vec2--ZERO)
           (camera_zoom 1.0)
           (last_mouse Vec2--ZERO)
           (layout_dirty true)
           (is_animating true)
           (last_layout_time (Instant--now))
           (hovered_path_cache (String--with_capacity 256))
           (tree_files 0)
           (tree_bytes 0)
           (layout_workspace (make-instance LayoutWorkspace
                                            :areas (Vec--new)
                                            :row (Vec--new)
                                            )
                             )
           )
       (declare (mutable watcher_mgr camera_pos camera_zoom last_mouse
                         layout_dirty is_animating last_layout_time
                         hovered_path_cache tree_files tree_bytes
                         layout_workspace
                         )
                )
       )
    )
  )
(defun emit-gui-scan-drain ()
  (list
    `(while-let ((Ok (ScanEvent--Batch batch)) (dot scan_rx (try_recv)))
                (when (merge_scan_batch (ref-mut root_node)
                                        (ref target_dir)
                                        batch
                                        (Some (ref-mut watcher_mgr))
                                        )
                  (= layout_dirty true)
                  )
                )
    )
  )
(defun emit-watcher-removal ()
  `(attr "allow(clippy::collapsible_if)"
                                          (if-let ((Some parent)
                                                   (dot root_node (find_mut parent_comps)))
                                                  (if-let ((Some pos)
                                                           (dot (dot parent children)
                                                                (iter)
                                                                (position
                                                                 (lambda (c)
                                                                   (== (dot c name) item_name)
                                                                   )
                                                                 )
                                                                )
                                                           )
                                                          (progn
                                                            (dot (dot parent children)
                                                                 (remove pos)
                                                                 )
                                                            (= (dot parent is_sorted) false)
                                                            (= layout_dirty true)
                                                            )
                                                          )
                                                  )
                                          )
  )
(defun emit-watcher-dir-spawn ()
  `(let ((deep_tx (dot scan_tx_keep (clone)))
         (deep_abort (dot abort_scan (clone)))
         (deep_target (dot path (clone)))
         (deep_scanners (dot active_scanners (clone)))
         )
     "thread::spawn(move || { scan_directory_recursive(&deep_target, &deep_tx, &deep_abort); deep_scanners.fetch_sub(1, std::sync::atomic::Ordering::Release); });"
     )
  )
(defun emit-watcher-dir-event ()
  `(let ((dir_needs_scan
                                                           (if-let ((Some parent)
                                                                    (dot root_node
                                                                         (find_mut parent_comps)
                                                                         )
                                                                    )
                                                                   (not (dot (dot parent children)
                                                                              (iter)
                                                                              (any
                                                                               (lambda (c)
                                                                                 (== (dot c name)
                                                                                     item_name
                                                                                     )
                                                                                 )
                                                                               )
                                                                              )
                                                                        )
                                                                   false
                                                                   )
                                                           )
                                                          )
                                                      (when dir_needs_scan
                                                        (if-let ((Some parent)
                                                                 (dot root_node
                                                                      (find_mut parent_comps)
                                                                      )
                                                                 )
                                                                (progn
                                                                  (dot (dot parent children)
                                                                       (push
                                                                        (FileNode--new
                                                                         (dot item_name
                                                                              (into_owned)
                                                                              )
                                                                         true
                                                                         0
                                                                         )
                                                                        )
                                                                       )
                                                                  (= (dot parent is_sorted) false)
                                                                  (= layout_dirty true)
                                                                  )
                                                                )
                                                        (dot watcher_mgr
                                                             (register_dir (ref path))
                                                             )
                                                        (dot active_scanners
                                                             (fetch_add 1 Ordering--Release)
                                                             )
                                                        ,(emit-watcher-dir-spawn)
                                                        )
                                                      )
  )
(defun emit-watcher-file-event ()
  `(when (dot meta (is_file))
                                                      (let ((new_size (dot meta (len))))
                                                        (if-let ((Some diff)
                                                                 (dot root_node
                                                                      (update_file_size
                                                                       (ref components)
                                                                       new_size
                                                                       now
                                                                       )
                                                                      )
                                                                 )
                                                                (when (!= diff 0)
                                                                  (= layout_dirty true)
                                                                  )
                                                                (if-let ((Some parent)
                                                                         (dot root_node
                                                                              (find_mut
                                                                               parent_comps
                                                                               )
                                                                              )
                                                                         )
                                                                        (progn
                                                                          (dot (dot parent children)
                                                                               (push
                                                                                (FileNode--new
                                                                                 (dot item_name
                                                                                      (into_owned)
                                                                                      )
                                                                                 false
                                                                                 new_size
                                                                                 )
                                                                                )
                                                                               )
                                                                          (= (dot parent is_sorted)
                                                                             false
                                                                             )
                                                                          (= layout_dirty true)
                                                                          )
                                                                        )
                                                                )
                                                        )
                                                      )
  )
(defun emit-watcher-metadata ()
  `(if-let ((Ok meta) (fs--metadata (ref path)))
     (if (dot meta (is_dir))
         ,(emit-watcher-dir-event)
         ,(emit-watcher-file-event)
         )
     )
  )
(defun emit-watcher-ok-arm ()
  `((Ok event)
                   (when (matches! (dot event kind)
                                   (space (scope EventKind Access) "(_)"))
                     (continue)
                     )
                   (for (path (dot event paths))
                        (when (is_virtual_or_special_fs (ref path))
                          (continue)
                          )
                        (if-let ((Ok rel) (dot path (strip_prefix (ref target_dir))))
                                (let ((components (dot rel (iter) (collect))))
                                  (declare (type "Vec<&OsStr>" components))
                                  (when (dot components (is_empty))
                                    (continue)
                                    )
                                  (let ((parent_comps
                                         (dot (dot components
                                                   (split_at (- (dot components (len)) 1))
                                                   )
                                              0
                                              )
                                         )
                                        (item_name
                                         (dot (aref components
                                                    (- (dot components (len)) 1)
                                                    )
                                              (to_string_lossy)
                                              )
                                         )
                                        (is_exists (dot path (exists)))
                                        )
                                    (if (not is_exists)
                                    ,(emit-watcher-removal)
                                    ,(emit-watcher-metadata)
                                        )
                                    )
                                  )
                                )
                        )
                   )
  )
(defun emit-watcher-err-arm ()
  `((Err e)
                   (eprintln! (string "[Watcher Event Error] {e}"))
                   )
  )
(defun emit-gui-watcher-drain ()
  (list
    `(while-let ((Ok res) (dot (dot watcher_mgr rx) (try_recv)))
                (case res
                  ,(emit-watcher-ok-arm)
                  ,(emit-watcher-err-arm)
                  )
                )
    )
  )
(defun prelude-wrap (body)
  `(let ((collected (dot (std--env--args) (collect))))
     (declare (type "Vec<String>" collected))
     (let (((tuple target_dir root_name) (resolve_target (ref collected))))
       (let ((root_node (FileNode--new root_name true 0))
             (active_scanners (Arc--new (AtomicUsize--new 1)))
             (abort_scan (Arc--new (AtomicBool--new false)))
             ((tuple scan_tx scan_rx) (channel))
             )
         (declare (mutable root_node))
         (let ((scan_tx_keep (dot scan_tx (clone))))
           (let ((spawn_target (dot target_dir (clone)))
                 (spawn_abort (dot abort_scan (clone)))
                 (spawn_tx (dot scan_tx_keep (clone)))
                 (spawn_scanners (dot active_scanners (clone)))
                 )
             ;; HATCH: move closure as a whole spawn statement (as in 03).
             "thread::spawn(move || { scan_directory_recursive(&spawn_target, &spawn_tx, &spawn_abort); spawn_scanners.fetch_sub(1, std::sync::atomic::Ordering::Release); });"
             )
           (let ((watcher_mgr (WatcherManager--new (ref target_dir)))
                 (camera_pos Vec2--ZERO)
                 (camera_zoom 1.0)
                 (last_mouse Vec2--ZERO)
                 (layout_dirty true)
                 (is_animating true)
                 (last_layout_time (Instant--now))
                 (hovered_path_cache (String--with_capacity 256))
                 (tree_files 0)
                 (tree_bytes 0)
                 (layout_workspace (make-instance LayoutWorkspace
                                                  :areas (Vec--new)
                                                  :row (Vec--new)
                                                  )
                                   )
                 )
             (declare (mutable watcher_mgr camera_pos camera_zoom
                               last_mouse layout_dirty is_animating
                               last_layout_time hovered_path_cache
                               tree_files tree_bytes layout_workspace
                               )
                      )
             ,@body
             )
           )
         )
       )
     )
  )
(defun frame-wrap (body)
  `(let ((dt (dot (get_frame_time) (min 0.05)))
         (screen_w (screen_width))
         (screen_h (screen_height))
         (world_canvas (Rect--new 0.0 44.0 screen_w (dot (- screen_h 44.0) (max 1.0))))
         (now (Instant--now))
         )
     ,@body
     )
  )
(defun layout-wrap (body)
  `(let ((scanning (< 0 (dot active_scanners (load Ordering--Acquire)))))
     (when (and layout_dirty
                (or (not scanning)
                    (< 100 (dot (dot last_layout_time (elapsed)) (as_millis)))
                    )
                )
       (= (dot root_node target_rect) world_canvas)
       (let (((tuple f_count b_count) (sum_tree_stats (ref-mut root_node))))
         (= tree_files f_count)
         (= tree_bytes b_count)
         )
       (layout_treemap_sequential (ref-mut root_node) (ref-mut layout_workspace))
       (= layout_dirty false)
       (= is_animating true)
       (= last_layout_time (Instant--now))
       ,(lprint :msg "layout"
                :vars '(tree_files tree_bytes))
       )
     (if scanning
         (snap_tree (ref-mut root_node))
         (when is_animating
           (= is_animating (update_animations (ref-mut root_node) dt now))
           )
         )
     ,@body
     )
  )
(defun input-wrap (body)
  `(let ((mouse_screen (Vec2--from (mouse_position))))
     (when (or (is_mouse_button_down MouseButton--Left)
               (is_mouse_button_down MouseButton--Middle)
               )
       (let ((delta (/ (- mouse_screen last_mouse) camera_zoom)))
         (incf camera_pos delta)
         )
       )
     (= last_mouse mouse_screen)
     (let ((wheel (dot (mouse_wheel) 1)))
       (when (< 0.01 (dot wheel (abs)))
         (let ((mouse_world_before (- (/ mouse_screen camera_zoom) camera_pos)))
           (if (< 0.0 wheel)
               ;; HATCH: assign-ops emit redundant parens (unused_parens),
               ;; x = x op y trips assign_op_pattern; exact strings do neither.
               "camera_zoom *= 1.15;"
               "camera_zoom /= 1.15;"
               )
           (= camera_zoom (dot camera_zoom (clamp 0.05 100.0)))
           (let ((mouse_world_after (- (/ mouse_screen camera_zoom) camera_pos)))
             (incf camera_pos (- mouse_world_after mouse_world_before))
             )
           )
         )
       (when (is_key_pressed KeyCode--Space)
         (= camera_pos Vec2--ZERO)
         (= camera_zoom 1.0)
         )
       ,@body
       )
     )
  )
(defun draw-wrap (body)
  `(progn
     (clear_background (Color--new 0.08s0 0.09s0 0.12s0 1.0s0))
     (let ((view_min (- camera_pos))
           (view_max (- (/ (Vec2--new screen_w screen_h) camera_zoom) camera_pos))
           )
       (render_treemap (ref root_node) view_min view_max camera_pos camera_zoom)
       (dot hovered_path_cache (clear))
       (let ((mouse_world (- (/ mouse_screen camera_zoom) camera_pos))
             (hovered_node (if (>= (dot mouse_screen y) 44.0)
                               (find_hovered_path (ref root_node)
                                                  mouse_world
                                                  (ref-mut hovered_path_cache)
                                                  )
                               None
                               )
                           )
             )
         ,@body
         )
       )
     )
  )
(defun hud-text-wrap (body)
  `(let ((watcher_alive (and (dot (dot watcher_mgr watcher) (is_some))
                             (dot (dot watcher_mgr error_msg) (is_none))
                             )
                        )
         )
     (let ((status_color (if scanning
                             (Color--new 0.95s0 0.8s0 0.1s0 1.0s0)
                             (if (dot watcher_mgr limit_hit)
                                 (Color--new 1.0s0 0.55s0 0.0s0 1.0s0)
                                 (if watcher_alive
                                     (Color--new 0.2s0 0.85s0 0.3s0 1.0s0)
                                     (Color--new 0.9s0 0.2s0 0.2s0 1.0s0)
                                     )
                                 )
                             )
                         )
           )
       (let ((status_text (if scanning
                              (dot (string "SCANNING...") (to_string))
                              (if (dot watcher_mgr limit_hit)
                                  (format!
                                    (string "WATCHING (INOTIFY LIMIT: {} DIRS)")
                                    (dot (dot watcher_mgr watched_dirs) (len))
                                    )
                                  (if watcher_alive
                                      (backend_label (ref watcher_mgr))
                                      (format!
                                        (string "WATCHER FAILED: {}")
                                        (dot (dot (dot watcher_mgr error_msg)
                                                  (as_deref)
                                                  )
                                             (unwrap_or (string "Unknown error"))
                                             )
                                        )
                                      )
                                  )
                              )
                          )
             )
         ,@body
         )
       )
     )
  )
(defun hud-status-wrap (body)
  `(progn
     (draw_rectangle 0.0 0.0 screen_w 44.0
                     (Color--new 0.06s0 0.07s0 0.09s0 0.95s0)
                     )
     (draw_line 0.0 44.0 screen_w 44.0 1.0
                (Color--new 0.15s0 0.17s0 0.22s0 1.0s0)
                )
     ,(hud-text-wrap
        (append
           (list
             `(draw_text (ref status_text) 14.0 18.0 15.0 status_color)
             `(draw_text (format! (string "Files: {}") tree_files)
                         310.0 18.0 15.0 WHITE
                         )
             `(draw_text (format! (string "Size: {}") (format_bytes tree_bytes))
                         440.0 18.0 15.0
                         (Color--new 0.2s0 0.8s0 1.0s0 1.0s0)
                         )
             )
           body
           )
         )
     )
  )
(defun hud-hover ()
  (list
    `(if-let ((Some node) hovered_node)
             (let ((info (format! (string "{} ({})")
                                  hovered_path_cache
                                  (format_bytes (dot node size_bytes))
                                  )
                       )
                   )
               (declare (mutable info))
               (when (< 1024.0 (dot node growth_rate))
                 (dot info
                      (push_str
                       (ref (format! (string "  [Active: {}/s]")
                                     (format_bytes (coerce (dot node growth_rate) u64))
                                     )
                            )
                       )
                      )
                 )
               (stmt (draw_text (ref info) 14.0 36.0 14.0
                                (Color--new 0.9s0 0.9s0 0.9s0 1.0s0)
                                )
                     )
               )
             (progn
               (draw_text (string "Pan: Left/Middle Drag | Zoom: Wheel | Reset: Space | Target: ")
                          14.0 36.0 14.0 GRAY
                          )
               (stmt (draw_text (dot (dot target_dir (to_string_lossy)) (as_ref))
                                420.0 36.0 14.0 LIGHTGRAY
                                )
                     )
               )
             )
    `(stmt (await (next_frame)))
    )
  )
(defun emit-gui-hud ()
  (list (hud-status-wrap (hud-hover)))
  )
(defun emit-gui-main ()
  `(defun-async gui_main ()
     ,(prelude-wrap
        (list
          `(loop
             ,(frame-wrap
                (append (emit-gui-scan-drain)
                        (emit-gui-watcher-drain)
                        (list (layout-wrap
                                (list (input-wrap
                                        (list (draw-wrap (emit-gui-hud))))))))))
          )
        )
     )
  )
(defun emit-gui ()
  (append
    (emit-backend-label)
    (list (emit-gui-main))
    )
  )
