# Cohors

A TUI music player written in Rust.

## Features
- File system navigation and playback (MP3, WAV, OGG, FLAC)
- Audio visualization (Spectrum Analyzer)
- Internet Radio support (Configurable via config file)
- Supports MPRIS (media playback controls on a system-wide level)
- Built-in management for Radio Stations and Sources

## Disclaimer

I'm happy to hear your feedback and feature requests! I'm also very open to PRs that add or enhance functionality. But, please, know that I created this application for myself and my friends, our feedback and needs come before any others.

## Key Bindings

| Key | Action |
| --- | --- |
| `q` | Quit Application |
| `TAB` | Toggle Mode (Files / Radio / Favorites) |
| `/` | Search / Filter |
| `?` | Toggle Help / About |
| `j` / `↓` | Move Selection Down |
| `k` / `↑` | Move Selection Up |
| `Enter` | Play Selection / Enter Directory |
| `Backspace` | Go Up Directory or Delete Station/Source |
| `Delete` | Delete Station/Source |
| `Space` | Toggle Pause / Resume |
| `+` / `=` | Volume Up |
| `-` | Volume Down |
| `→` | Next Track |
| `←` | Previous Track |
| `l` | Toggle Loop Mode (Off / Track/ Folder) |
| `h` | Toggle hidden files in File View Mode |
| `x` | Export selected radio station to config |
| `a` | Add Station/Source |
| `e` | Edit Station/Source |
| `f` | Toggle Favorite |

## Command Line Arguments

`cohors [OPTIONS] [PATH]`

| Argument | Description |
| --- | --- |
| `-v`, `--volume <0-100>` | _(Optional)_ Set the initial volume (default: 100) |
| `-r`, `--radio` | _(Optional)_ Start in Radio mode |
| `--invalidate-cache` | Force the station list to be re-downloaded |
| `-h`, `--help` | Print help information |
| `-V`, `--version` | Print version information |
| `[PATH]` | _(Optional)_ Path to a file or directory to play on startup |

## Managing Stations & Sources

You can manage your radio stations and sources directly within the application without manually editing the JSON configuration file.

### Adding Items
Press `a` to open the Add menu. You will be prompted to choose what to add:
- Press `s` to add a **Single Station**.
- Press `r` to add a **Source** (a dynamic list of stations from a JSON URL).

### Example - Adding a station from radio-browser.info

1. Visit www.radio-browser.info and craft your search there. When finished, click the JSON button and copy the URL provided, for example: `https://de2.api.radio-browser.info/json/stations/search?limit=10&name=80s&hidebroken=true&order=clickcount&reverse=true`

2. Open Cohors.
2. Press `tab` until you reach the `Radio Stations` tab.
3. Press `a` to add a new station/source.
4. Press `r` to add a radio source and use these fields:

   - Title: Anything you want
   - JSON URL: The above URL
   - Container: Leave empty, no mapping override is needed, the default matches.
   - Map: Name - "name"
   - Map: URL - "url"
   - Map: Desc - "description"
   - Map: Home - "homepage"
   - Map: Tags - "tags"

5. Press `Enter` to save and your category and stations should show up in the Radio Stations list

### Editing Items
To edit an existing item, navigate to it in the Radio list and press `e`.
- **Custom Stations**: You can edit any station that you've added manually (under "Custom Stations").
- **Sources**: Select the source header (the group title) to edit the source configuration.

### Deleting Items
To delete an existing item, navigate to it in the Radio list and press `backspace` or `delete`.
- **Custom Stations**: You can delete any station that you've added manually (under "Custom Stations").
- **Sources**: Select the source header (the group title) to delete the source configuration.

### Input Dialogs
When adding or editing, a dialog will appear with several fields.
- **Navigation**: Use `Tab` / `Down` to move to the next field, and `Shift+Tab` / `Up` to move back.
- **Saving**: Press `Enter` to save your changes.
- **Canceling**: Press `Esc` to close the dialog without saving.

Changes are automatically saved to your `config.json` file.

## Radio Configuration (Manual)

While the built-in UI handles most tasks, you can still manually configure sources in `config.json`. The station data is downloaded and cached for one week. To invalidate the cache and force a re-download, use the `--invalidate-cache` argument.

The application looks for the configuration file in the following order:
1. `$XDG_CONFIG_HOME/cohors/config.json`
2. `~/.config/cohors/config.json`
3. `./config.json`

### Configuration Format

The configuration file is a JSON object containing an optional `radio` object, which holds lists of `stations` and/or `sources`.

Each station defines an individual station you want to list while each source defines where to fetch the data and how to map the JSON fields to Cohors' internal station structure.

Stations support the following fields:

 - `name`: Display name of the station
 - `station_url`: The URL to stream from
 - `description` (Optional): A description of the station.
 - `homepage` (Optional): The station's home page.

*Radio  stations can be export to your individual station list by pressing `x`. This is useful if you're using a JSON link that could have rotating results based on properties.*

Sources support the following fields:

- `title`: Display name for this group of stations in the UI.
- `json_url`: The URL to fetch the JSON data from.
- `container` (Optional): If the station list is inside a property of the root JSON object, specify the key here. If the root is an array, omit this field.
- `mapping`: Maps the API's field names to Cohors' fields. Nested fields can be accessed using dot notation (e.g., `playlists.0.url`).

```json
{
  "radio": {
    "stations": [
      {
        "name": "My Favorite Station",
        "station_url": "http://stream.example.com/radio",
        "description": "Best hits 24/7",
        "homepage": "http://example.com"
      }
    ],
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
}
```


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


### Screenshots

<details>
   <summary>Files View</summary>

<img width="1723" height="725" alt="image" src="https://github.com/user-attachments/assets/807dc773-fdf7-4894-b185-dd3372e861eb" />

</details>

<details>
   <summary>Station View</summary>

<img width="1723" height="725" alt="image" src="https://github.com/user-attachments/assets/0b8dc80c-a060-4424-aedf-5cc86600adbd" />

</details>

<details>
   <summary>Add/Edit Modal</summary>

<img width="1714" height="720" alt="image" src="https://github.com/user-attachments/assets/22680c1b-1d10-4504-a81f-72f0dbc6f9c0" />

</details>
