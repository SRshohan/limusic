## Why

`http-search-vertical-slice` proved `crates/innertube` works unmodified behind a plain HTTP
server, and the frontend can reach it — but only for one command. Outside Tauri, every page except
`/offline` still redirects or crashes: the root route (`ui/src/routes/+page.ts`) redirects straight
to `/offline` because the real `/search` page also calls `searchAll()`, which (like every other
browse/library command) is an explicit `unsupported(...)` stub in `ui/src/lib/api.web.ts`. This
change ports the rest of the **anonymous, read-only** browse/search surface — home feed, search
beyond songs, and album/artist/playlist viewing — so a plain browser tab can actually show YouTube
Music content, not just redirect away from it. Signed-in features (library, liked songs, history,
playlists-you-own, multi-account) are a separate, later change: they need a credential-custody
decision (server-side session storage vs. client-held cookie) this change deliberately does not
make, matching how `http-search-vertical-slice`'s design.md scoped auth out of its own slice.

## What Changes

- Add HTTP routes to `crates/server` for every anonymous browse/search command the frontend already
  has a typed stub for, reusing `crates/innertube`'s existing methods unmodified (same
  `metadata_client`/`state.it` pattern the existing `/api/search` route already uses):
  - Search: `searchAll`, `searchSuggestions`, `searchCards`, `searchVideos` — all already real
    `InnerTube` methods (`search_all`, `search_suggestions`, `search_cards`, `search_videos`).
  - Browse: `getHome`/`getHomeMore` (home feed + continuation), `getAlbum`, `getArtist`,
    `getPlaylist`/`getPlaylistMore` (viewing only, not the write actions on the same page),
    `getMoods`/`getMoodArt`, `getBrowseGrid`.
- Replace the matching `unsupported(...)` stubs in `ui/src/lib/api.web.ts` with real `fetch` calls
  against the new routes, following the exact pattern `search` already established there. No change
  to `api.tauri.ts`, `api.ts`'s dispatcher, or any call site — every component already calls through
  `api.ts`, so swapping the web implementation is invisible to them.
- `getAlbum`/`getPlaylist` drop the local-SQLite-backed branches their Tauri-command counterparts
  have (`local::ALBUM_PREFIX`/`ARTIST_PREFIX` in `get_album`, `ON_REPEAT_ID` in `get_playlist`):
  those ids only exist on the desktop app's own SQLite library, which this server has no access to
  and the browser's local-music feature (`ui/src/lib/local/`) never produces.
- Once `/search`'s `searchAll` call stops throwing, remove the `/offline`-only redirect in
  `ui/src/routes/+page.ts` and let the real home page render outside Tauri too, same as inside it.
- Nothing about playback changes: `play`, `playIndex`, `playPlaylist`, `startRadio`, queue state,
  and video/audio streaming stay explicit `unsupported(...)` stubs. Viewing a page and actually
  playing something from it are different problems — streaming audio into a browser tab needs its
  own design (a media proxy route, buffering, MediaSource/HTMLAudioElement wiring) and is
  deliberately a separate, later change.
- Nothing about sign-in changes: every new route is anonymous-only, exactly like the existing
  search route. A route that only makes sense signed in (library, history, owned playlists,
  write actions) is left as `unsupported(...)` until that separate change happens.

## Capabilities

### New Capabilities
- `browse-http-api`: resolving a YouTube Music browse target (the home feed, an album, an artist,
  a playlist, the Moods & Genres tiles, a browse grid) to its content over a plain HTTP request,
  anonymously, independent of any specific client.

### Modified Capabilities
- `search-http-api`: broadens the existing anonymous search capability from songs-only
  (`GET /api/search`) to the full search surface the desktop app already has — categorized
  all-results search, typeahead suggestions, per-category "show more" cards, and video search.

## Impact

- `crates/server` gains routes for every command listed above; no new dependencies expected (same
  `axum`/`innertube` stack as the existing route).
- `ui/src/lib/api.web.ts` loses most of its `unsupported(...)` stubs for read-only browse/search
  commands; `ui/src/routes/+page.ts` loses its `!isTauri` redirect once `/search` no longer crashes.
- `crates/innertube` is unmodified — every method this change calls already exists
  (`endpoints.rs`: `search_videos`, `search_all`, `search_suggestions`, `search_cards`, `home`,
  `home_continuation`, `moods`, `playlist`, `album`, `artist`, `browse_grid`,
  `playlist_continuation`).
- Still nothing in `src-tauri`; the desktop build is unaffected, same as the prior slice.
