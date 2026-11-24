# Rust Learning App

A local web application (Axum) that teaches you Rust step by step, through code you write in your browser. Student code is compiled and run with `rustc` inside a container, so Rust does not need to be installed on the host machine.

## Requirements

- Windows 11 + [Podman Desktop](https://podman-desktop.io/) (the WSL machine must be running)
- `podman compose` (included with Podman Desktop) or `podman-compose`

## Quick start

```powershell
.\run.ps1
```

The image is built, the container is started, and `http://localhost:8080` is printed. Other options: `-Build`, `-Up`, `-Down`, `-Logs`, `-Check`, `-Shell`.

## Content structure

Lessons are located under `content/`. For the format and fields, see [content/SCHEMA.md](content/SCHEMA.md).

## Language (Turkish / English)

Select a language from the flag menu at the top right; the choice is stored in the `lang` cookie. Routes and progress are shared between both languages. The Turkish files are the primary source; the English texts are in files with the same names under `content/en/levelN/` (see [SCHEMA.md](content/SCHEMA.md), section `## İngilizce içerik`). UI strings are in `src/i18n.rs`.

## Points and unlock system (short)

Every task awards points when solved correctly. The next lesson or level stays locked until the required number of points is reached.

## Progress file and backup

Progress is stored in `data/progress.json` (`./data` is bind-mounted into the container). To back it up, copy the file; to restore it, put the file back in the same place. To reset progress, delete the file.

## Content check

```powershell
.\run.ps1 -Check
```

Runs `/app/rust_learning_app --check-content`, which validates the content files.

## Security

The port is exposed only for local access (`127.0.0.1:8080`); other devices on the network cannot reach it. The container is restricted: read-only root filesystem, tmpfs for `/tmp`, all capabilities dropped, and memory, CPU and process limits. **However, student code can reach the external network.** Disabling networking entirely would also disable port publishing, so it was not disabled. Use the app only on your own machine and do not expose it to the internet.

## WSL bind mount note

The `./data` folder is mounted through WSL. If you run into write permission problems:

- keep the `:Z` label in `compose.yaml`;
- in `compose.yaml`, remove the leading `#` from the `# userns_mode: keep-id` line (via CLI: `podman run --userns=keep-id ...`).

## Troubleshooting

- The first build takes a long time (dependencies are downloaded and compiled); later builds are faster thanks to the cache.
- If you use Git Bash, run `export MSYS_NO_PATHCONV=1` before Podman commands.
- If `podman compose` is not found, update Podman Desktop or install `podman-compose`.
- If port 8080 is in use, change the port mapping in `compose.yaml` (e.g. `127.0.0.1:8081:8080`).
- The Rust image is pinned to the `1.99-slim` tag; to update it, change the two `FROM` lines in `Containerfile`.
