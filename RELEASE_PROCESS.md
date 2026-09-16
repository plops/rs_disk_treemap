# Release Process

This repository is configured to automatically build and publish release binaries using GitHub Actions. Whenever a new Git tag is pushed, the workflow will compile both the `treemap_mvp` and `treemap-disk-analyzer` projects for Windows and Linux, and upload them to a GitHub Release.

## How to create a new release

### Step 1: Bump the version numbers
Before releasing, make sure the versions in your `Cargo.toml` files are up to date.

1. Open `00_mvp/Cargo.toml` and update the `version = "X.Y.Z"` if necessary.
2. Open `01_more/Cargo.toml` and update the `version = "X.Y.Z"` if necessary.
3. Commit these changes:
   ```bash
   git add .
   git commit -m "chore: bump version for release"
   git push origin main
```
