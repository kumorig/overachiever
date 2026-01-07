# Overachiever - Achievement Progress Tracker for Steam Games

An application for tracking your Steam game library and achievement progress over time.  
Demo: https://overachiever.space/IHh1wBke but mainly available as a desktop app. Releases [here](https://github.com/kumorig/steam-overachiever-v3/releases).


(This project is not affiliated with steam or endorsed by Valve Corporation in any way or form.)

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
- [ ] post time to beat. 
- [ ] post comments (tag multiple achievements).
- [ ] upload progress added in last version doesn't show progress. (you had one job)
- [ ] charts by 1w,1m,3m,6m,1y,max (or similar)
- [ ] list recently added user games to some list
- [ ] fix profile button (icon, naming, intent), it's confusing.
- [ ] CJK font option
- [ ] (important for the future!!): If we ever get a few more users, and more than one user scans at the same time, we will hit rate limits. We should implement some kind of queue. And run a backend service to handle requests. -- Currently if you leave the app, requests will stop, but the backend could keep going. its also a problem to let the client trigger requests indescriminately. 
- [ ] Improve privacy policy parts. It's not very clear what data is stored or sent where.

## License
This project is licensed under the MIT License. See the `LICENSE` file for details.

## Acknowledgements
This project is in no way affiliated with or endorsed by Valve Corporation.
