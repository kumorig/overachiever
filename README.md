# Overachiever v3

An application for tracking your Steam game library and achievement progress over time. 

This project is in no way affiliated with or endorsed by Valve Corporation.


## Setup
For it to work, you need to provide your Steam API key and Steam ID. You can configure these in the app by clicking the ⚙ (settings) button in the top-right corner.

To get a Steam API key, visit [Steam Web API Key](https://steamcommunity.com/dev/apikey) and register (free).

Your Steam ID is a 17-digit number. You can find it by visiting your Steam profile and looking at the URL, or use a site like [steamid.io](https://steamid.io/) or [steamidcheck.com](https://steamidcheck.com).

Configuration is stored in `config.toml` in the same directory as the executable.


## Building
Make sure you have [Rust](https://rust-lang.org/tools/install/) installed. Then, run:

```bash
cargo run --release
```
or 
```bash
cargo build --release
```

## Contributing
Contributions are welcome. Make a PR or open an issue. 
About half of the code has been "vibe-coded", feel free to help clean-up any mess. AI contributions are welcome, but at least do some low effort testing before submitting a PR. Thanks!

## Roadmap
Feel free to open an issue with suggestions.
[ ] Improve the graphs, but lets run the app a few weeks so we have some data to work with first.
[ ] Add utility to help you figure out your steam-id.
[ ] backend for comments ratings on achievements. To help users find the easy ones.
[ ] Pack icon files into a single binary blob/texture atlas to reduce file count. Option to not cache icons to disk.
[ ] Feature: export of achievement data to CSV/JSON.


## License
This project is licensed under the MIT License. See the `LICENSE` file for details.

## Acknowledgements
This project is in no way affiliated with or endorsed by Valve Corporation.