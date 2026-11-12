# Releasing StoneAge 2

Releases are built by GitHub Actions and delivered to installed apps by the built-in updater
(sidebar section **Updates**, crate `crates/stoneage-update`).

## One-time setup on GitHub

1. Create the repository and push the code. The updater checks
   `stoneage_update::UPDATE_REPO` in `crates/stoneage-update/src/lib.rs`
   (currently `Tamer-Mansour/stoneage2`). If the repository gets another name, change that one
   constant. For testing, a single installation can be pointed elsewhere with
   `UpdateRepo = owner/name` in `StoneAge.ini` next to the exe.
2. The repository must be **public**. The updater calls the GitHub API without a token, and a
   private repository returns 404, which the app reports as "no release published".
3. Settings → Actions → General → Workflow permissions: **Read and write permissions**, so
   `release.yml` can create the release. The workflow also requests `contents: write` itself.

## Making a release

1. Bump the version in the root `Cargo.toml`:
   ```toml
   [workspace.package]
   version = "2.1.0"
   ```
   Run `cargo build` once so `Cargo.lock` is updated, then commit both files.
2. Tag the commit with the same version and a `v` prefix, then push the tag:
   ```
   git tag v2.1.0
   git push origin v2.1.0
   ```
   The workflow stops if the tag and the Cargo version differ.
3. `.github/workflows/release.yml` then:
   - builds `stoneage.exe` for `i686-pc-windows-msvc` (x86) and `x86_64-pc-windows-msvc` (x64),
     linking the MSVC runtime statically,
   - packages `stoneage-windows-x86.zip` and `stoneage-windows-x64.zip`. Each zip holds
     `stoneage.exe`, `README.md` if present, and a `licenses/` folder with the root `LICENSE*`
     files and the icon license `crates/stoneage-app/assets/icons/LICENSE-tlp-icons.txt`,
   - writes `SHA256SUMS.txt` covering both zips,
   - creates the GitHub Release with these three assets and generated release notes. You can
     edit the notes on GitHub afterwards; the app shows them as plain text.
4. Tags with a hyphen, such as `v2.2.0-beta.1`, are published as **pre-releases**. Apps see them only
   when "Include pre-releases" is switched on.

Do not rename the assets. The updater looks for these exact file names.

## What the installed apps do

- They check `GET https://api.github.com/repos/<repo>/releases/latest` on startup (on by default;
  the "Check automatically on startup" setting turns it off), or when the user clicks **Check now**.
  The request sends only the User-Agent `StoneAge2-Updater`, with no personal data. Unauthenticated
  requests are limited to 60 per hour per IP address. When the limit is reached, the app says so
  and changes nothing.
- When the tag is newer than the running version (semver), the app shows a toast and a badge on the
  Updates icon. It does nothing else until the user acts.
- **Download** fetches the zip that matches the build (x86 or x64) and checks it against
  `SHA256SUMS.txt`. If the checksum is missing or does not match, the download is deleted and
  nothing is installed.
- **Install & restart** first runs the usual unsaved-changes prompt. It then extracts the package
  next to the exe, renames the running exe to `stoneage.exe.stoneage-old`, puts the new exe in its
  place and copies the other files. An existing `StoneAge.ini` is never overwritten. Finally it
  starts the new version and closes the old one. The new process deletes the `*.stoneage-old`
  files.
- **Skip this version** hides the startup notification for that version only.

The install folder must be writable by the user. A portable folder works; `C:\Program Files`
without admin rights does not. If the folder is not writable, the app reports an error and leaves
the installation unchanged.

## Testing the updater locally

- `cargo test -p stoneage-update` runs the unit tests for version comparison, asset selection,
  checksum parsing and install/rollback on temporary folders.
- `cargo test -p stoneage-update -- --ignored` runs a live check against a repository that does
  not exist. It must report `NoRelease`, not panic.
- To test end to end, publish a release with a higher version in a test repository. Set
  `UpdateRepo = you/test-repo` in `StoneAge.ini` next to an older build, then use Check now,
  Download and Install & restart.
