# Local rules for zucchini-rs

- Every commit gets a DCO sign-off: always pass `--signoff` (`-s`) to `git commit`, so the message
  ends with `Signed-off-by: <git user.name> <git user.email>`.
- Every commit Claude makes also ends with the trailer
  `Co-Authored-By: <model> <noreply@anthropic.com>` (after the sign-off).
- Semantic commit names (`fix:`, `docs:`, `build:`, `ci:`, `style:`), short subject, body of at
  most a line or two. One commit per fix.
- Releases: follow the release guide in `AGENTS.md`.
