;; gen.lisp --- loader for the transpiled treemap MVP.
;;
;; Run from the repo root:
;;   sbcl --load 03_mvp_lisp/gen.lisp --quit
;; Writes:
;;   03_mvp_lisp/src/main.rs   (committed generated code)
;;
;; Pattern: cl-rust-generator/examples/21_mandelbrot/gen00.lisp
;; Language reference: cl-rust-generator/SUPPORTED_FORMS.md
(eval-when (:compile-toplevel :execute :load-toplevel)
  (ql:register-local-projects)
  (ql:quickload "cl-rust-generator")
  )
(in-package :cl-rust-generator)
(setf *rustfmt-arguments* '("--edition" "2024"))
(defparameter *project-dir*
  (make-pathname
    :name nil
    :type nil
    :version nil
    :defaults (merge-pathnames #P"./" *load-pathname*)
    )
  )
(defparameter *code-file*
  (merge-pathnames #P"src/main.rs" *project-dir*)
  )
(load (merge-pathnames #P"lisp/helpers.lisp" *project-dir*))
(load (merge-pathnames #P"lisp/scan.lisp" *project-dir*))
(load (merge-pathnames #P"lisp/layout.lisp" *project-dir*))
(load (merge-pathnames #P"lisp/render.lisp" *project-dir*))
(load (merge-pathnames #P"lisp/main.lisp" *project-dir*))
(let ((*omit-redundant-parens* t))
  (write-source
    *code-file*
    `(do0
       (use
         (macroquad prelude *)
         (std ffi OsStr)
         (std fs)
         (std path (curly Path PathBuf))
         (std sync mpsc channel)
         (std thread)
         )
       (defstruct0 Node
         (path PathBuf)
         (size u64)
         (is_dir bool)
         (children "Vec<Node>")
         (rect Rect)
         (color Color)
         )
       ,(report-skip-fn)
       ,(emit-scan-tree)
       ,(emit-scan-entry)
       ,(emit-worst)
       ,(emit-layout-row)
       ,(emit-squarify)
       ,(emit-hash-color)
       ,(emit-color-for-path)
       ,(emit-format-bytes)
       ,@(emit-format-bytes-test)
       ,(emit-render-tree)
       ,(emit-main)
       )
    )
  )
