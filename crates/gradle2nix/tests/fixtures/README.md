# gradle2nix Test Fixtures

All fixtures are recorded against **Gradle 8.4.0**. See `tapi-schema.json` for the TAPI output schema.

## Directory Structure

```
fixtures/
├── tapi-schema.json              TAPI JSON schema + version pin
├── tapi-outputs/                 Pre-recorded TAPI shim JSON outputs
├── gradle-projects/              Minimal Gradle project skeletons
├── maven-coords/                 Maven coordinate string test inputs
├── maven-repos/                  Offline stub Maven repositories
│   ├── maven-central-stub/       Valid artifacts with .sha256 files
│   ├── corrupt-sha256/           Artifact with invalid hex in .sha256
│   └── missing-artifact/         Empty — all lookups return ENOENT
├── lockfiles/                    Sample gradle.nix lockfiles
└── nix-outputs/                  Expected Nix expression outputs
```

## TAPI Outputs

| File | Description |
|------|-------------|
| `basic.json` | 2 deps: guava (compile) + junit (test) |
| `with-classifiers.json` | guava + guava:sources + asm (3 artifacts) |
| `with-test-scope.json` | 4 deps: 2 compile + 2 test scope |
| `malformed-missing-field.json` | `{}` — empty object, triggers serde "missing field" error |
| `malformed-unknown-fields.json` | Valid + extra `buildId` field (forward compat test) |
| `version-mismatch.json` | `version: "99.0.0"` — triggers unsupported version error |

## Gradle Projects

Each project has `build.gradle`, `settings.gradle`, and `.gradle2nix-tapi-output.json`
(pre-recorded TAPI output sidecar — set via `TapiShimConfig::tapi_json_override` in tests).

| Project | Description |
|---------|-------------|
| `simple-app/` | 1 module, 2 deps (guava + junit), no classifiers |
| `multi-module-2level/` | 2 submodules (:app, :lib), 5 transitive deps |
| `with-classifiers/` | 1 module, deps with sources classifier |

## Maven Repos

The `maven-central-stub/` directory is a local offline Maven repository.
Each artifact has a `.sha256` file containing only the 64-char hex digest.

SHA-256 values are synthetic test data — not real artifact hashes.

## Lockfiles

| File | Description |
|------|-------------|
| `simple-2-deps.json` | 2 deps: guava + junit (canonical form) |
| `simple-2-deps-stale.json` | Same as above but guava sha256 changed |
| `complex-20-deps.json` | 20 deps with mixed scopes |
| `malformed-invalid-json.json` | Unparseable JSON |
| `malformed-missing-sha256.json` | Valid JSON but LockedDependency missing `sha256` |

## Fixture Maintenance

If the TAPI schema changes (e.g., Gradle upgrade), update fixtures via:
```
./scripts/record-tapi-fixtures.sh  # Phase 2 — not yet implemented
```
