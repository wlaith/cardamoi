<picture>
  <source media="(prefers-color-scheme: dark)" srcset="brand/assets/social/readme-banner-dark-1280x320@1280.png">
  <img alt="cardamoi" src="brand/assets/social/readme-banner-light-1280x320@1280.png">
  

https://github.com/user-attachments/assets/9a854214-7848-4683-be60-ab0b609e622c


</picture>

# cardamoi

A desktop app for finding, downloading and watching films and TV. It keeps a local library, plays it in [mpv](https://mpv.io) with resume and next-episode autoplay, and finds subtitles from OpenSubtitles and Subdl.

Built with [Tauri 2](https://tauri.app), Vue 3 and Rust. Torrents are handled by [librqbit](https://github.com/ikatson/rqbit); search goes through a bundled [Prowlarr](https://prowlarr.com) or any Torznab endpoint you add. Catalog data comes from [TMDB](https://www.themoviedb.org).

Builds for macOS (Apple Silicon and Intel), Windows and Linux are on the [releases page](https://github.com/wlaith/cardamoi/releases). It is primarily tested on macOS (Apple Silicon).

macOS builds aren't notarized yet, so the first time you open the app, right-click it and choose Open (or run `xattr -dr com.apple.quarantine /Applications/cardamoi.app`).

## Requirements

- Node.js 20+
- Rust (stable)
- [mpv](https://mpv.io/installation/), on your `PATH`:
  - macOS: `brew install mpv`
  - Windows: `scoop install mpv` or `choco install mpv`, or download a build from mpv.io and add its folder to `PATH`
  - Linux: your package manager, e.g. `sudo apt install mpv`, `sudo pacman -S mpv`, or `sudo dnf install mpv` (Fedora needs RPM Fusion)

## Setup

```sh
npm install
./scripts/fetch-prowlarr.sh          # osx-arm64 by default; or osx-x64, linux-x64, windows-x64
npm run tauri dev
```

On first launch the app asks for your own free TMDB key (from https://www.themoviedb.org/settings/api) and walks you through getting one. You can change it later in Preferences > TMDB.

Downloads go to `~/Movies/cardamoi`. Videos you copy into that folder yourself are picked up and added to the library.

## Tests

```sh
cd src-tauri && cargo test
```

## Releasing

Pushes and pull requests to `development` and `main` run CI on macOS, Windows and Linux. Releases are cut from `main`:

1. On `development`, run `./scripts/bump-version.sh 0.2.0` and merge that.
2. Open a pull request from `development` to `main` and merge it.
3. Tag the release: `git fetch && git tag v0.2.0 origin/main && git push origin v0.2.0`

The tag starts the release workflow. It checks that the tag is on `main` and matches the app version, builds every platform into a draft release, and publishes it once every build has uploaded. A tag with a suffix, like `v0.2.0-beta.1`, is published as a pre-release.

## Disclaimer

cardamoi does not host or index any content. You are responsible for what you download and for complying with the laws where you live.

This product uses the TMDB API but is not endorsed or certified by TMDB.
