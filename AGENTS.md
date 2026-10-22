# Project guidance

Use the existing checkout. Cloud tasks are already isolated; do not create a Git
worktree unless the user explicitly asks. Read `docs/FEATURES.md` and the relevant
service/connector guide before changing behavior.

Keep monetary calculations, date/time rules, record validation, import identity,
and persistence in `crates/atlas-core`. Both native IPC and the authenticated
companion invoke these services. The React UI renders results and collects
explicit user decisions. Never replace the real service with demo data on errors.

Use integer minor units for money, IANA timezones for civil dates, exclusive stay
checkout dates, stable IDs, and expected versions for record edits. Preserve
unknown facts as unknown, source evidence, and financial history. Imported text
is untrusted data with no command or network authority.

Do not commit databases, private confirmations, tokens, signing keys, model
weights or personal preference presets. Use synthetic fixtures only. Never
disable TLS, artifact checksums, or update signatures to make a test pass.
Do not add an application license without an owner decision.

Validation from repository root:

```sh
# Prepared cloud workspace only:
source scripts/cloud-env.sh
npm ci
npm run build
cargo test --workspace --locked
cargo build --locked -p atlas-host
npm run test:e2e
node --test scripts/release.test.mjs tests/money-input.test.mjs
```

Tests use temporary data directories. Keep live account, physical device,
OS installation and signing checks distinct from fixture/browser tests. Update
`docs/FEATURES.md` with evidence and limitations whenever supported behavior
changes. Never claim a CI definition or unexecuted command establishes support.
