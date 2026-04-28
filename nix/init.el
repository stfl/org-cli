;;; init.el --- Minimal Emacs init for org-cli live tests  -*- lexical-binding: t; -*-
;;
;; Standalone, repo-local Emacs config used by the live-test daemon.
;; Packages are provided by Nix via `emacsWithPackages'; nothing is fetched at
;; runtime. Designed for an isolated, ephemeral daemon — never touches the
;; user's ~/.config/emacs or any system Emacs configuration.
;;
;; The launcher script (or the rstest fixture) is responsible for setting:
;;   - ORG_LIVE_DIR      : org-directory for the daemon (optional)
;;   - ORG_LIVE_FILES    : colon-separated org files to expose
;;
;; Both default to no-op when unset, so this init.el also evaluates cleanly
;; under `emacs --batch -l init.el' in CI smoke checks.

;;; Code:

(setq inhibit-startup-screen t
      initial-scratch-message nil
      ring-bell-function 'ignore)

(require 'org)

(require 'mcp-server-lib)
;; `mcp-server-lib-start' lives in mcp-server-lib-commands.el; in a daemon
;; the autoload covers this, but be explicit so `emacs --batch -l init.el'
;; smoke checks work too.
(require 'mcp-server-lib-commands)
(require 'org-mcp)

(unless mcp-server-lib--running
  (mcp-server-lib-start))

;; Wire org-mcp's GTD query API to minimal org-ql expressions. Real production
;; setups use agile-gtd helpers, but for live tests we just need org-mcp's
;; query-* tools to return SOMETHING deterministic against arbitrary org files
;; — no projects, no rank, no agile-gtd dependency. Tests that need richer
;; semantics layer an overlay via `emacs -l <overlay.el>`.
;;
;; Signatures (see `org-mcp-query-*-fn' docstrings):
;;   inbox-fn   : zero args.
;;   next-fn    : one optional tag-filter sexp (e.g. `(tags "x")') or nil.
;;   backlog-fn : same as next-fn.
(setq org-mcp-query-inbox-fn
      (lambda () '(and (todo) (tags "inbox")))
      org-mcp-query-next-fn
      (lambda (&optional tag-filter)
        (if tag-filter `(and (todo "NEXT") ,tag-filter) '(todo "NEXT")))
      org-mcp-query-backlog-fn
      (lambda (&optional tag-filter)
        (let ((base '(and (todo "TODO") (not (tags "inbox")))))
          (if tag-filter `(and ,base ,tag-filter) base))))

;; Pull org-directory and the allow-list from the environment so the launcher
;; / rstest fixture can drive everything from outside Emacs. Per-test fixtures
;; that need GTD query semantics (org-mcp-query-inbox-fn etc.) load their own
;; overlay file via -l after this init.el — agile-gtd is intentionally NOT a
;; dependency of the base env.
(let ((dir (getenv "ORG_LIVE_DIR")))
  (when (and dir (not (string-empty-p dir)))
    (setq org-directory (expand-file-name dir))))

(let ((files-env (getenv "ORG_LIVE_FILES")))
  (when (and files-env (not (string-empty-p files-env)))
    (let ((files (mapcar #'expand-file-name
                         (split-string files-env ":" t))))
      (setq org-agenda-files       files
            org-mcp-allowed-files  files))))

(message "org-cli-live: init loaded (org-directory=%s, %d agenda files)"
         (or org-directory "<unset>")
         (length (or org-agenda-files '())))

(provide 'init)
;;; init.el ends here
