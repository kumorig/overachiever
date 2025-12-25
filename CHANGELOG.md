### 0.3.4
* No longer require login to view libraries other people have linked.

### 0.3.3 - 2025-12-25
* allow updating single games with refresh-button in game table.
* fix for games not being added automatically when appearing in recently played. (probably free games which weren't added by full scan?)
* fix uneven striped background in the log-view.
* launch button for installed games (in desktop version).
* We now find installed apps by:
  - Finding Steam installation path via registry or common paths
  - Parsing libraryfolders.vdf to find all Steam library folders
  - Scanning for appmanifest_*.acf files in steamapps folders
* new filter for installed games (desktop only).
* install button for non installed games(desktop only).
* sharable user-url (to enable, login/logout in wasm, or unlink/link in desktop).
* upload progress for "upload data" (desktop only).

### 0.3.2
* fix for unlocked games history being overwritten.
* fix/hide current build warnings, so we get a clean build for release.
* added db field for keeping track of total unplayed games (regardless of achievements) in run history.
* added build-number and date to tooltip for the title "Overachiever" on desktop (already existed in wasm).

### 0.3.1 and earlier
* There is no changelog for 0.3.1 and earlier versions.
