# Version

## Usage

```
Usage: version [OPTIONS] [VERSION_BUMP]

Arguments:
  [VERSION_BUMP]  If not provided, configured to read from git, will attempt to infer the bump from the git commit message, else `prepatch` [possible values: prepatch, patch, preminor, minor, major, skip]

Options:
      --from-git             Infer version bump from git commit messages
      --workspace            Expect to find a workspace rather than a normal project
      --message-file <FILE>  Path to commit message file
      --path <FILE>          Path to manifest file [default: Cargo.toml]
      --quiet                Suppress all output except errors
  -h, --help                 Print help
  -V, --version              Print version
```

## Expected behavior

| Current Version | Bump Applied | Next Version |
| - | - | - |
| 1.2.3 | Major | 2.0.0 |
| 1.2.3 | Minor | 1.3.0 |
| 1.2.3 | Patch | 1.2.4 |
| 1.2.3-alpha.0 | Patch | 1.2.3 |
| 1.2.3-alpha.0 | Minor | 1.2.0 |
| 1.2.3-alpha.0 | Major | 2.0.0 |
| 1.2.3 | Prepatch | 1.2.4-alpha.0 |
| 1.2.3 | Preminor | 1.3.0-alpha.0 |
| 1.2.4-alpha.1 | Prepatch | 1.2.4-alpha.2 |
| 1.3.0-alpha.2 | Preminor | 1.3.0-alpha.3 |

## How I use it:

```sh
# .git/hooks/prepare-commit-msg

VERSION_BIN="target/__version"

if [ ! -f $VERSION_BIN ]; then
    # Download to VERSION_BIN
fi

# Only run for regular commits
if [ -z "${2:-}" ] || [ "$2" = "message" ]; then
    echo "Running version bump..."

    if $VERSION_BIN --from-git --message-file "$1" --workspace-manifest Cargo.toml; then
        if ! git diff --quiet Cargo.toml; then
            # Mark that we need to amend the commit
            touch .git/NEED_VERSION_AMEND
        fi
    else
        exit 1
    fi
fi

```

```sh
# .git/hooks/post-commit
set -euo pipefail

if [ -f .git/NEED_VERSION_AMEND ]; then
    rm -f .git/NEED_VERSION_AMEND

    git add Cargo.toml
    git commit --amend --no-edit
fi
```
