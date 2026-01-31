# Overachiever - Achievement Progress Tracker for Steam Games

An application for tracking your Steam game library and achievement progress over time.  

* The desktop version is a standalone application that runs on your computer. It will scan your recently played Steam games on every app launch, and draw a nice litte graph if you have enough data. It stores all data locally in a SQLite database file.

* The WASM (web) version allows you to publish your game/achievement data online. This is not automatic, you have to opt-in and publish your data manually (a button-press in cloud options). It will not update automatically, and you have to re-upload data every time to refresh your online profile. 

* A steam openID login is required to upload/remove your data. 

* No login is required to view published profiles. 

* Demo: https://overachiever.space/IHh1wBke. Releases [here](https://github.com/kumorig/steam-overachiever-v3/releases).



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

## Roadmap (so I don't forget :>)
- [ ] tag multiple achievements with comments.
- [ ] hide games (import from steam hidden games)
- [ ] type up vdf parser properly
- [ ] add a button to open config from options.
- [ ] upload progress added in last version doesn't show progress. (you had one job)
- [ ] charts by 1w,1m,3m,6m,1y,max (or similar)
- [ ] list recently added user games to some list
- [ ] Improve privacy policy parts. It's not very clear what data is stored or sent where.

## License
This project is licensed under the MIT License. See the `LICENSE` file for details.

## Acknowledgements
This project is in no way affiliated with or endorsed by Valve Corporation.
