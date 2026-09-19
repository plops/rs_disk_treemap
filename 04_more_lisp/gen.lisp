;; gen.lisp --- loader for the transpiled treemap analyzer (01_more).
;;
;; Run from the repo root:
;;   sbcl --load 04_more_lisp/gen.lisp --quit
;; Writes:
;;   04_more_lisp/src/main.rs   (committed generated code)
;;
;; Pattern: 03_mvp_lisp/gen0.lisp after
;;   cl-rust-generator/examples/21_mandelbrot/gen00.lisp
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
(load (merge-pathnames #P"lisp/data.lisp" *project-dir*))
(load (merge-pathnames #P"lisp/layout.lisp" *project-dir*))
(load (merge-pathnames #P"lisp/scan.lisp" *project-dir*))
(load (merge-pathnames #P"lisp/watcher.lisp" *project-dir*))
(load (merge-pathnames #P"lisp/format.lisp" *project-dir*))
(load (merge-pathnames #P"lisp/headless.lisp" *project-dir*))
(load (merge-pathnames #P"lisp/render.lisp" *project-dir*))
(load (merge-pathnames #P"lisp/gui.lisp" *project-dir*))
(load (merge-pathnames #P"lisp/tests.lisp" *project-dir*))
(load (merge-pathnames #P"lisp/main.lisp" *project-dir*))
(let ((*omit-redundant-parens* t))
  (write-source
    *code-file*
    `(do0
       ,(emit-uses)
       ,(report-skip-fn)
       ,@(emit-data)
       ,@(emit-layout)
       ,@(emit-scan)
       ,@(emit-watcher)
       ,@(emit-format)
       ,@(emit-headless)
       ,@(emit-render)
       ,@(emit-gui)
       ,@(emit-tests)
       ,@(emit-main)
       )
    )
  )
