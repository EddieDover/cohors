# Cohors

A TUI music player written in Rust.

## Features
- File system navigation and playback (MP3, WAV, OGG, FLAC)
- Audio visualization (Spectrum Analyzer)
- Internet Radio support (SomaFM)

## Command Line Arguments

`cohors [OPTIONS] [PATH]`

| Argument | Description |
| --- | --- |
| `-v`, `--volume <0-100>` | Set the initial volume (default: 100) |
| `-r`, `--radio` | Start in Radio mode |
| `-h`, `--help` | Print help information |
| `-V`, `--version` | Print version information |
| `[PATH]` | Path to a file or directory to play on startup |

## Key Bindings

| Key | Action |
| --- | --- |
| `q` | Quit Application |
| `TAB` | Toggle Mode (Files / Radio) |
| `?` | Toggle Help / About |
| `j` / `↓` | Move Selection Down |
| `k` / `↑` | Move Selection Up |
| `Enter` | Play Selection / Enter Directory |
| `Backspace` | Go Up Directory |
| `Space` | Toggle Pause / Resume |
| `+` / `=` | Volume Up |
| `-` | Volume Down |
| `→` | Next Track |
| `←` | Previous Track |
| `l` | Toggle Loop Mode (Off / Track/ Folder) |
| `h` | Toggle hidden files in File View Mode |

## Attribution
This application uses the SomaFM API to provide radio channels.
**SomaFM** is a listener-supported, commercial-free internet radio station.
Please support them at [somafm.com](https://somafm.com).
