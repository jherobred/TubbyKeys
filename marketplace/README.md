# Sound pack marketplace

Everything in this folder is the catalogue TubbyKeys shows on its Marketplace page. The app downloads `index.json` and the pack files straight from this repository over HTTPS. Before installing, it checks each file's size and SHA-256 against the index.

## Add a pack

1. Fork the repository.
2. Create `marketplace/packs/<your-pack-id>/`. The id uses lowercase letters, digits and dashes, for example `gateron-yellow-pbt`.
3. Add your sound files and a `pack.json` (format below).
4. Run `npm run marketplace` to validate the pack and regenerate `index.json`.
5. Commit both your pack folder and the updated `index.json`, then open a pull request.

CI runs the same validation. A maintainer listens to the pack before merging.

### Test it locally first

Copy your pack folder into the TubbyKeys packs folder (Settings → Marketplace → **Open packs folder**). Then reopen the settings window. The pack appears under **Community**, where you can hover to preview it and click to use it.

## Rules

- **Only sounds you have the right to share.** Record them yourself, or use sounds under CC0, CC-BY-4.0, CC-BY-SA-4.0, MIT or Apache-2.0. Do not rip audio from videos, games or other apps.
- Set `license` to the pack's license and credit every original author in `author` and `credits`.
- Audio: WAV, MP3, OGG (Vorbis) or FLAC, at most 4 seconds per file. Trim silence before the click. TubbyKeys trims it too, but tight files load faster.
- At most 64 sound files, 2 MB per file and 16 MB per pack.
- Only `pack.json` and the files it references. No other files in the folder.

## pack.json

```json
{
  "schema": 1,
  "id": "my-pack",
  "name": "My Pack",
  "version": "1.0.0",
  "author": "Your Name",
  "license": "CC0-1.0",
  "type": "tactile",
  "description": "One or two sentences about the board and switches.",
  "source": "https://link-to-original-recordings.example",
  "press": {
    "rows": [["press/r0.wav"], ["press/r1.wav"], ["press/r2.wav"], ["press/r3.wav"], ["press/r4.wav"]],
    "space": ["press/space.wav"],
    "enter": ["press/enter.wav"],
    "backspace": ["press/backspace.wav"]
  },
  "release": {
    "default": ["release/generic.wav"],
    "space": ["release/space.wav"]
  },
  "credits": [
    { "file": "press/space.wav", "author": "Someone", "url": "https://example.com/original" }
  ]
}
```

| Field | Required | Notes |
| --- | --- | --- |
| `schema` | yes | Always `1`. |
| `id` | yes | Must match the folder name. |
| `name` | yes | Shown in the app, up to 64 characters. |
| `version` | yes | `major.minor.patch`. Bump it when you change the pack so users see **Update**. |
| `author`, `license` | yes | See the rules above. |
| `type` | yes | `linear`, `tactile`, `clicky` or `other`. |
| `description`, `source`, `credits` | no | Descriptions are up to 300 characters. |
| `press` | yes | Sounds for key down. Needs `default` or `rows`. |
| `release` | no | Sounds for key up. Leave it out if your clips include the release. |

### Sound classes

Each class is a list of files. With more than one file, TubbyKeys picks one at random per key press.

- `default`: any key.
- `rows`: up to five lists, from the number row (0) to the space bar row (4). Real boards sound different per row, so this is the most realistic option.
- `space`, `enter`, `backspace`: the big keys.

If a pack has no sound for a big key, TubbyKeys plays the row or default sound pitched slightly lower.

Paths may use one folder level, for example `press/r0.wav`. They may contain only letters, digits, `-` and `_`.
