# Cohors

A TUI music player written in Rust.

## Features
- File system navigation and playback (MP3, WAV, OGG, FLAC)
- Audio visualization (Spectrum Analyzer)
- Internet Radio support (Configurable via radio.json)

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

## Command Line Arguments

`cohors [OPTIONS] [PATH]`

| Argument | Description |
| --- | --- |
| `-v`, `--volume <0-100>` | _(Optional)_ Set the initial volume (default: 100) |
| `-r`, `--radio` | _(Optional)_ Start in Radio mode |
| `-s`, `--station-file <PATH>` | _(Optional)_ Path to the station configuration file |
| `-h`, `--help` | Print help information |
| `-V`, `--version` | Print version information |
| `[PATH]` | _(Optional)_ Path to a file or directory to play on startup |

## Radio Configuration

Cohors supports internet radio by fetching station lists from JSON APIs. You can configure multiple sources in `stations.config.json`.

The application looks for the configuration file in the following order:
1. The path specified by `--station-file <PATH>`
2. `~/.config/cohors/stations.config.json`
3. `./stations.config.json`

### Configuration Format

The configuration file is a JSON object containing a list of `sources`. Each source defines where to fetch the data and how to map the JSON fields to Cohors' internal station structure.

```json
{
  "sources": [
    {
      "title": "Example Radio Source",
      "json_url": "https://api.example.com/stations.json",
      "container": "stations",
      "mapping": {
        "station_name": "name",
        "station_url": "stream_url",
        "description": "desc",
        "homepage": "website",
        "tags": "genre",
        "lastPlaying": "current_song"
      }
    }
  ]
}
```

- `title`: Display name for this group of stations in the UI.
- `json_url`: The URL to fetch the JSON data from.
- `container` (Optional): If the station list is inside a property of the root JSON object, specify the key here. If the root is an array, omit this field.
- `mapping`: Maps the API's field names to Cohors' fields. Nested fields can be accessed using dot notation (e.g., `playlists.0.url`).

### Example API Response

For the configuration above, the `json_url` (`https://api.example.com/stations.json`) would be expected to return a JSON structure similar to this:

```json
{
  "stations": [
    {
      "name": "Cool Radio",
      "stream_url": "http://stream.example.com/cool",
      "desc": "The coolest beats",
      "website": "https://coolradio.example.com",
      "genre": "Jazz, Funk",
      "logo_url": "https://coolradio.examplecom/logo.png",
      "current_song": "Miles Davis - So What"
    },
    {
      "name": "News FM",
      "stream_url": "http://stream.example.com/news",
      "desc": "24/7 News",
      "website": "https://newsfm.example.com",
      "genre": "News, Talk",
      "logo_url": null,
      "current_song": "Breaking News"
    }
  ]
}
```

The `container` field is set to `"stations"`, telling Cohors to look inside that property for the list. The `mapping` then connects fields like `name` to `station_name` and `stream_url` to `station_url`.

