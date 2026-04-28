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
(require 'org-habit)
(require 'agile-gtd)
(require 'org-edna)
(org-edna-mode 1)

(require 'mcp-server-lib)
;; `mcp-server-lib-start' lives in mcp-server-lib-commands.el; in a daemon
;; the autoload covers this, but be explicit so `emacs --batch -l init.el'
;; smoke checks work too.
(require 'mcp-server-lib-commands)
(require 'org-mcp)

(unless mcp-server-lib--running
  (mcp-server-lib-start))

;; Wire org-mcp's query API to agile-gtd helpers, mirroring the production
;; setup so live tests exercise the same code path.
(setq org-mcp-ql-extra-properties
      '((parent-priority . agile-gtd--direct-parent-priority)
        (rank            . agile-gtd--item-rank))
      org-mcp-query-inbox-fn   #'agile-gtd-agenda-query-inbox
      org-mcp-query-backlog-fn #'agile-gtd-agenda-query-backlog
      org-mcp-query-next-fn    #'agile-gtd-agenda-query-next-actions
      org-mcp-query-sort-fn    #'agile-gtd--item-rank<)

;; Pull org-directory and the allow-list from the environment so the launcher
;; / rstest fixture can drive everything from outside Emacs.
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
