# What's new in Melue

Each version: what changed, newest first. Shown in the app (About: What's new), and used as each release's notes.
A new version adds its section at the top (the version commit's message).

## 1.0.0 (2026-10-28)

Melue 1.0: your own music on your computer and your phone, finished and documented.

- A new front page on GitHub: what Melue does, with screenshots of both, where to download it, how to set it up,
  what goes online, and answers to common questions. Melue is now open source under the MIT license.
- The light theme's page is a softer grey, a little lighter than before, so the white cards stand out without
  contrast that's too strong.
- On a phone, the status bar and navigation bar icons follow Melue's theme: dark icons on the light theme, where
  they used to stay white and disappear.
- Melue now reports its real version in its log and to the services it uses (it said 0.6.3).
- Melue can be built from source without the release signing key.

## 0.8.2 (2026-10-28)

Steadier and lighter, after a full check of the app.

- Lighter on a phone: the player reports its place less often (twice a second on screen, once in the background),
  and the page only redraws when something changed. The song's progress line keeps moving while playing.
- Phones that can play some songs on their own audio chip only with a short gap between songs get a setting for it
  (Settings, Audio: Save battery on … songs). Most phones play everything without gaps and never see it.
- More stable: a part of Melue that fails in the background starts again by itself instead of stopping for good.
- Faster with big libraries: the Library reads alongside a scan instead of waiting for it, a changed music folder is
  scanned on its own, and pages you left update only when you come back.
- A thin line no longer flashes above the mini player when switching pages on a phone.

## 0.8.1 (2026-10-25)

Melue on the keyboard, and lighter on your phone's battery.

- The arrows move around the app (computers): ↑ / ↓ change page, → goes into it (where you were) and the arrows go
  to the nearest thing that way; ← from the left edge or Esc goes back out. Shift+↑ / ↓ change page from anywhere,
  Shift+← / → go back or on 5 seconds. In a song, → goes to its buttons (✕ takes it out of Up next).
- M opens the menu of what's selected, and its items show which one is chosen. Shift+Enter picks a song up to move
  it in Up next or a playlist (↑ / ↓, then Enter).
- Much less work while nothing plays (a phone used about a third of a core, now almost nothing), and less while
  playing: the playing bars, the seek bar and the lyrics move without redrawing the page all the time.
- Up next no longer jumps on opening Now playing; a song added from a toast is shown there; Space plays and pauses
  on full-size lyrics; toasts show on a phone's Now playing.

## 0.8.0 (2026-10-24)

Spotify playlists in Melue, and your music folders followed as they change.

- Spotify playlists (Playlists: Add from Spotify): paste a public playlist's link, no sign-in or Premium needed. Its
  songs are matched to the ones you have; the others show greyed, and their list can be copied. It follows the
  playlist on Spotify by itself, with Spotify's own picture as its cover (or its songs' covers, if you like).
- A playlist with more than 100 songs gets its full list from a CSV (Update list: the link is copied and the exporter
  opens; on a computer Melue picks the download up by itself). A "!" says when it changed on Spotify since.
- Spotify playlists sync to your other devices like your own playlists, with your changes (songs removed show greyed,
  with Restore). Stop following Spotify keeps it as your own playlist, on every device.
- Keep only on this device asks what happens on your other devices: keep their copy, or delete it from them (syncing
  it again brings it back).
- On a computer, Melue watches your music folders: a song added, changed or removed shows up by itself, and fills in
  a playlist's greyed song.
- Songs added to or removed from a playlist on another device unfold and fold away in place.
- Toasts can be tapped: "Added to …" opens the playlist with the song pointed out, a song added to the queue opens Up
  next, and others open where they're about. On a phone, toasts show on Now playing too.

## 0.7.2 (2026-10-24)

Discord, and smoother queues.

- Discord (Settings: Connections; computers): your status says what you're listening to in Melue, with the song, its
  artist, how far it is and the album's cover (looked up online). Hovering the cover can show this week's listening.
- Continue: another device playing a song this one doesn't have is continued anyway; the song shows as playing there,
  and play starts the next song here (Across devices: "Songs this device doesn't have").
- Connecting to a device that plays while this one doesn't just joins in: what plays there goes on.
- Up next: tapping a song plays it with the queue as it was; songs added unfold into place, and changes from the other
  device glide (a song moved, the queue going on).
- Now playing shows a note where a song has no cover.

## 0.7.1 (2026-10-24)

Playing on another device, more fully; your songs across devices; sign-ins everywhere.

- Playing on another device: lists play that device's own copy (a playlist, an album, an artist, Liked songs, Top
  songs; its own songs in them too), and Up next is edited from here (Play next, Add to queue, move, remove, clear).
  Songs and lists only on this device are refused, saying so. On a computer playing on a phone, the media keys and
  Windows' media panel control the phone.
- Songs on all your devices, each its own copy but not on the share: the Library card offers to put them there (the
  best copy goes; Ask, Automatically or Off). Songs put in the synced folder by hand count as only on this device.
- The Library card's rows open their songs: tick some or all, then Forget (songs that can't be found), Move, or Put on
  share. Music folders list a folder's songs not on the share, those on all your devices marked; a tap ticks a song.
- Across devices: an old device (a Melue removed) can be forgotten, so it isn't counted anywhere.
- Signing in to GitHub on one device signs in on all of them (through the share, encrypted with its password), and
  signing out does too.
- The phone app stays upright (portrait). A folder that stopped being the synced one gets its own name back.

## 0.7.0 (2026-10-23)

Play on your other devices: your phone on your computer's speakers, or the other way round, like Spotify Connect for
your own music.

- Connect device (the player bar's speaker button, Now playing, a drawer on the phone): what plays here carries on
  on the device you pick, and this one becomes its remote: the song, the controls, the seek bar, Up next and the
  volume are that device's. "This device" brings it back; either device can disconnect, and the music goes on.
- A pill where toasts show says where the music is ("Playing on …", "Played from …"); tap it for Connect device.
- Settings, Across devices: your devices and which are online; "Let other devices play here" (On, Ask first, Off);
  the volume's steps on a phone and, on a phone, its volume buttons' steps on a computer.
- Playing on a phone, the volume is the phone's own, in its steps. On the phone, the notification, the lock screen,
  the media card and the volume buttons control the device it plays on.
- Likes show on both devices at once while connected; a dropped connection comes back by itself within a minute.
- Since 0.6.0: the volume shows as a number as it changes; the phone's lyrics follow every song again; pressing play
  just after Melue starts is no longer undone by Continue.

## 0.6.7 (2026-10-23)

The phone's own controls, for the device it plays on.

- Playing on another device from the phone, the notification, the lock screen and the media card show what plays
  there ("Playing on …"), and their buttons control it: play, pause, previous, next, seeking, shuffle and repeat. The
  heart likes the song playing there.
- The phone's volume buttons move the other device's volume (in its steps: "Volume steps on a phone", or "Volume
  buttons on another device" for a computer), not the phone's own. Android gives them over while it plays.
- While connected, a like or unlike on either device shows on the other at once.
- Fixed: on the phone, the lyrics stayed on one song after Now playing had been closed.
- Fixed: pressing play just after Melue started could be undone by Continue loading another device's playback.

## 0.6.6 (2026-10-23)

Connections that come back.

- A connection to another device that drops (a network change, Wi-Fi going and coming back) is tried again for a
  minute, the music playing on there meanwhile: the pill says "Reconnecting to …", then you're back in control. It
  ends sooner when Melue was closed on the other device.
- A device that asks first doesn't ask again when the one that was playing on it comes back.
- A device that can't be reached says why in words: not running Melue, or online but blocked (a firewall).

## 0.6.5 (2026-10-23)

The volume, across your devices.

- Playing on a phone, the volume is the phone's own media volume (what its buttons change). The slider, the mouse
  wheel and the + and − keys move it in steps, marked on the slider, and the number shows the step ("8/16"). The
  phone's own buttons show on the other device as you press them.
- Settings, Across devices: "Volume steps on a phone" (how many of a phone's levels a step moves; 10 by default, like
  the buttons on phones with fine volume) and, on a phone, "Volume buttons on another device" (how much a press moves
  a computer's volume, for the volume buttons to come).
- The volume set from another device stays when you disconnect.

## 0.6.4 (2026-10-23)

Connect device: play on your other devices.

- Connect device (the player bar's speaker button, Now playing's Connect device, a drawer on the phone): pick one of
  your devices, and what plays here carries on there, from where it was; this device pauses and becomes the remote.
  Its controls, the song, the seek bar and Up next show what plays there. Playing a song or a list plays it there.
- "This device" brings it back, with the queue it had; Disconnect leaves it playing there. The device played on can
  disconnect too, and goes on playing.
- A pill where toasts show says where the music is ("Playing on …" or "Played from …"); tapping it opens Connect
  device. Toasts come above it.
- "Ask first" asks on the device before another plays on it.
- One way at a time: a device played on that starts playing elsewhere disconnects the one playing on it first.
- A device that crashes or leaves the network is noticed within 15 seconds, and the connection ends.
- While connected, Continue (playback across devices) rests on both, and doesn't offer what was just played there.
- Two devices writing the same file on the share at once no longer pause the share's syncing.

## 0.6.3 (2026-10-23)

One device playing on another, under the hood.

- Your devices can play on each other: one sends the other what to play (its queue, the song and where in it) and
  controls it (play, pause, next, previous, seek, volume, shuffle, repeat), and sees what it plays as it changes.
  Each plays its own copies of the songs. Connect device, to use it, comes next.

## 0.6.2 (2026-10-23)

Your devices, and the first steps toward playing on them from each other.

- Settings, Across devices: your devices, which are online now (or when they were last seen), and "Let other
  devices play here": On, Ask first or Off.
- Your devices can reach each other over the network (at home or through a VPN), proving they're yours with a key
  on the share. On a computer, Windows asks once whether Melue may use the network.
- Other devices' changes to settings, likes and playlists arrive within seconds again: watching the share for
  changes restarts by itself when its connection drops.

## 0.6.1 (2026-10-23)

The volume on Now playing.

- On Now playing (computers), the volume's level shows beside its slider while it changes: with the mouse, or the
  + and − keys (which no longer show the player bar's volume at the window's edge, the bar being away there).

## 0.6.0 (2026-10-23)

Melue's first release: your own music on every device.

- Your music folders on a computer and a phone, and an SMB share (a NAS) that keeps them in sync: songs, likes,
  plays, playlists and settings, with Android Auto on the phone.
- Continue on one device what another was playing; lyrics, timed and tappable; playlists that follow their files.
- Melue updates itself on computers and phones, says what's new, and lets you report a bug from the app.
- Since 0.5.22: the "Continue from …" card shows the song's cover and the whole device name, and a song you put in
  the synced folder yourself (not on the share) no longer counts as one to move into the share's layout.

## 0.5.22 (2026-10-23)

A test of every setting, and what it found.

- Songs that move to another music folder keep their likes, plays and playlist places (choosing a synced folder
  inside a music folder made them look unavailable in playlists).
- Choosing where the synced music goes: Melue says when that folder already has songs (they'd be there twice).
- Continuing another device's playback starts on the right song when one before it can't play here.
- Devices no longer write their settings to the share over and over when they disagree on one neither changed.
- Phones: Melue no longer closes at start when a sync has nothing to do; putting new songs in place on the share no
  longer repeats every few seconds.
- Sync on share changes is shared across devices, and works again straight after switching it back on.
- Songs gone from the share that wait for your OK are shown on the sync card, not hidden in its details.
- The Top songs page sets the size of every Top songs list, as the setting does.
- Folders a sync leaves empty are removed; a folder that's no longer synced gets its own name back.

## 0.5.21 (2026-10-22)

Report a bug from Melue.

- Report a bug (About, or Ctrl+Shift+B): say what happened, then see the whole report before it's sent. It adds what
  helps find the problem: the version and device, the last steps, the screen described in words, errors, the sync
  and the library, and the log's last lines. Each part can be left out.
- Nothing personal goes out: song titles, artists, albums, playlists, folders, paths, the share's address and login,
  device names, addresses and links are replaced by placeholders like [song 1]; the review shows what each stands for
  (only to you).
- Reports are public issues on Melue's GitHub, sent from your own account: Settings has a new Connections section to
  sign in to GitHub. Not signed in, or offline: Send later keeps it, and it's sent by this device or another of yours
  that's signed in, through the share. About lists your reports.
- Keep a screenshot with reports (off unless switched on): kept with the report on your share, never sent.
- The player bar's song gives up its width gradually as the window narrows, before everything moves closer together.

## 0.5.20 (2026-10-22)

The keyboard everywhere, and smoother changes.

- Keyboard shortcuts (computers; ? lists them): play / pause, seeking, songs, volume (shown in the player bar), like,
  shuffle, repeat, the pages, Search, Up next (Q, and back where you were), the lyrics full size (Y). They work
  from anywhere except while typing; a menu or dialog closes first, and a question keeps the keys to itself.
- Tab goes through the page only (round from its end to its start), scrolling smoothly; a page switch starts at the
  top. In lists: arrows from song to song or card to card, Enter plays or opens, Shift+F10 the menu.
- Ctrl+↑ / ↓ scroll smoothly, in a page or a dialog.
- Smoother changes: tips, counts and a setting's line fade or slide as they change; the phone's tab marker slides;
  the player bar's spacing eases when the window narrows, and the progress keeps its room.
- Development and test builds are an app of their own, Melue Dev, with their own library and settings.

## 0.5.19 (2026-10-22)

Melue updates itself on phones too.

- Phones: Melue looks for a newer release like on a computer, asks (a drawer: Update, Not now, Don't ask again),
  downloads it and hands it to Android's installer. The first time, Android asks to allow Melue to install apps.
  Android installs only an update signed like the installed app.
- Releases carry the phone's APK too.

## 0.5.18 (2026-10-22)

What's new, and Melue updates itself.

- What's new: each version's changes, in About and in a note after an update.
- Updating itself (computers): Melue looks for a newer release a minute after start and every 6 hours, asks
  (Update, Not now, Don't ask again; "Ask when an update is found" in About), then downloads, installs and starts
  again. Only updates signed with Melue's key install. Releases come at the end of each phase.
- Lyrics: each tap's controls sit beside the lines in the full view; a dot marks a tapped line in the card.
- Errors can be copied: a right-click (or a long press) gives Copy, and Copy with troubleshooting details.
- Dialogs in the card colour, opening without a button focused; Escape closes them.
- Fixed: the library couldn't be read on a phone with a music folder named after its path.

## 0.5.17 (2026-10-22)

Lyrics, and the share is never a removable music folder.

- Lyrics on Now playing (a card as tall as the cover beside the controls, or a few lines under them; full size
  over the page with the top bar and the player bar): from a .lrc next to the song, its tags, or LRCLIB
  ("Find lyrics online", on Wi-Fi, on by default; looked up when a song plays and for new songs in the
  background; kept for offline). The line and the word being sung light up in two colours from the cover,
  picked to read well and to stand apart. Word times are estimated when the lyrics only time lines.
- Timing: earlier/later per song; "Sync by tapping" sets lines where they're tapped (the lines after follow),
  each tap fine-tunable or removable; timed lyrics from another version of the song are recognised and the
  next LRCLIB entry tried ("Try another version"); lyrics without times get a rough timing until tapped. The
  timing goes to the other devices with the library data.
- A music folder that is the share, inside it, or looks like it: Melue never removes files from it, leaves it
  out of Remove copies, and offers to take it out of the library.
- Hover tips in the app's style everywhere.

## 0.5.16 (2026-10-22)

Mobile data limits.

- On mobile data: Nothing (what syncs by itself, the library data and synced playback wait for Wi-Fi),
  Small things, or Everything within a monthly limit counted from the plan's billing day. When the limit is
  reached songs wait for Wi-Fi, or Melue asks; a sync that starts by itself asks before copying more than the
  chosen size. The sync card offers "Copy on mobile data" for what's waiting.
- This month's use: on the phone, and on a computer per phone (sent with each phone's library data).
- A sync copies what fits in what's left of the limit; the rest waits for Wi-Fi.

## 0.5.15 (2026-10-22)

Songs that can't be found, copies per folder, the share isn't a music folder.

- Library: a count of songs whose files can't be found, and Show for the list (folder, file, since when); hidden
  copies of synced songs per music folder, each with its own Remove.
- The synced folder, or a folder inside it, can't be added as a music folder; network folders are recognised
  (a mapped drive's \server\share), and one that is the share or inside it is refused; one around the share
  skips its part. Network folders are marked in the list.
- Removing songs from a network folder deletes them for good (no Recycle Bin there), and the questions say so.

## 0.5.14 (2026-10-22)

Settings that match across devices, logs, tray controls, Start on, setting links, resets with Undo.

- Settings: once everything has started and synced, each shared setting here is checked against the share's
  value and set to it; a difference at start-up no longer counts as a change made here. Resets (a row's, or a
  heading's "Reset these to default" for the section) go to the other devices and offer Undo. A setting's
  search highlight plays once.
- Melue's log on desktop too (%LOCALAPPDATA%\com.simon.melue\logs), shown from About; the phone shares its
  log through Android's share sheet.
- The tray menu: play / pause, next and previous (native-audio 1.10.0).
- Start on: Home, Library or where you left off. melue://settings/<id> links open Melue at a setting.
- A development build never switches on starting with Windows (nor through a reset).

## 0.5.13 (2026-10-21)

Phone drawers, empty states, sync fixes, faster start-up, pull to refresh.

- Phone: menus as drawers (⋯ and long press on songs, cards, list headers, Now playing, settings rows); setting
  choices and the Library sort as choice drawers; pull down on a main page to sync (or scan without a share).
- Empty and error states on every page, with Try again.
- Sync: Cancel works at once while the share is still being reached; coming back syncs after idling with the
  window in front, and doubled focus events no longer lose the time away; "12 min ago" texts share one live clock.
- Start-up: the desktop window shows once the UI has drawn; the first frame in the theme's colours; Home draws its
  last screen at once; Settings loads as its own chunk; the start-up scan and Android Auto's list wait for the first
  screen; start-up timings in the log.
- Fixes: no link dragging, the page-switch flash, settings reveals and separators.

## 0.5.12 (2026-10-19)

The phone's frame and layouts, settings across devices, playback fixes.

- The phone's own frame (MobileShell) around the same pages: top bar, bottom bar (Home, Library,
  Playlists, Search, Settings), a mini player, Now playing sliding up over everything. Pull Now
  playing down from anywhere, swipe a page from the side to the right, or back, to close. Back keeps
  one step of history, then sends Melue to the background (it keeps playing) instead of closing its
  screen. Shared page logic with the desktop frame (shells/usePages.ts).
- Phone sizes (a phone: style variant): two cards a row, smaller titles, slimmer song rows (a tap
  plays; the queue's ✕ in ⋯), stacked list headers, Library tabs on one line, Top songs' sizes on a
  row of their own, settings rows that wrap (a long title or description, controls under it), short
  share names. Pages opened on top are loaded ahead, so opening one doesn't stutter.
- Settings across devices (sync/settings_sync.rs, settings/sharedSettings.ts): every setting that
  isn't this device's own, newest change winning, through .melue/settings.json; a device's first
  snapshot never overrides the others; "This device only" turned off takes the shared value again.
- Playback: the phone restores its last queue at start-up; a sync looks at the other devices'
  playback again; the progress bar keeps a song's length through syncs, doesn't run on to the end
  after a while away, holds where you let go of it, slides on jumps, and lets the page scroll past it.
- Linked playlists: songs added in Melue show "Only in Melue" (and are counted). Add to playlist says
  how it went (with Undo). Badges centred on their words. Empty playlists aren't kept in Recently
  deleted. An artist's card has a person icon. Start with Windows is off-limits in development builds.

## 0.5.11 (2026-10-18)

Playlist covers, one-song play, drops on every page, cover sync, likes, volume.

- Playlist covers: Change cover… (a crop window: drag, zoom, a picture dropped on it), Revert to
  default cover; covers of synced playlists sync (newest change wins), also linked ones whose file is
  in the synced folder (sync/cover_sync.rs, migrations 14 and 15).
- Linked playlists' badges by where their file is: Synced or This device, with the link next to it.
- Playing a song: its play button plays just it (Queue setting "A song from All songs plays": Now /
  Next) and keeps Up next; a double-click adds it to the queue; only a list's big Play / Shuffle starts
  the list. Search's song cards: a play button on the cover. Notes for what went into the queue, with
  Undo. A queue of songs put together one by one repeats them all (native-audio 1.9.1).
- Dropping songs works on every page: one window asks where they go (the share, a music folder here,
  or the Music folder), copied into Artist/Album/; on a playlist's page they can go into it too
  (library/adding.rs). A fresh desktop install starts with the Music folder.
- Songs only on this device: a "This device" badge and Move to sync in their menu (asked first).
- The like animation (fill from the bottom, pop, burst), on every heart of the song at once.
- The player bar's volume: a button with a slider rising above it; mute on click, the wheel.
- Fixes: a page switch no longer lays out the new page under the old one (Library's rows came late,
  scroll jumped); Clear and removing the last song in Up next close smoothly; Up next shows the next
  pass on the last song with repeat all ("Starts over next"); the queue button scrolls to Up next as
  Now playing opens; the player keeps moving while Melue isn't the focused window (WebView2 flags);
  Home's playing card fades between songs; the drop window's padding and transitions.
- "…" is two dots everywhere; badges centred on their letters; Inter shipped with the app
  (@fontsource/inter, offline too); "When Windows starts it": in the tray or with its window.

## 0.5.10 (2026-10-18)

Home and Search.

- Home: the time of day; Playing and "Continue from <device>?" cards (primary or like the cards);
  Recently played lists (one row, See all) and Recent songs (they play as their own list).
- Search: songs, artists, albums, playlists and settings matching anywhere (accents and case
  ignored), an artist's songs and albums with them; See all per group; Recently added before
  typing; a skeleton for the first results; every change fades in.
- Library tabs fade between each other.
- Pages come back exactly where they were scrolled (scroll anchoring off); a setting link's
  flash doesn't come back; song rows: instant hover, the row under the pointer lights up when
  placing ends, a placed song's highlight background fades (words and icons at once).

## 0.5.9 (2026-10-18)

Add to playlist, Export as .m3u8, setting links in Search.

- Song menus: Add to playlist (a submenu: New playlist…, your playlists with ✓ on the ones that
  have the song); a new one opens with its name ready to type.
- Export as .m3u8… (desktop) for your playlists and Made for you: full paths, #EXTINF, UTF-8.
- Playlist files let go of: "File gone" with Forget when the file no longer exists.
- ⋯ on every list page (album, folder, playlist, Made for you) from the shared header.
- No blank page-on-top sliding away at start or on reload.
- Search: Enter on a pasted setting link (melue://settings/<id>) opens Settings at it.

## 0.5.8 (2026-10-18)

Playlists (the page, a playlist's page, Made for you pages).

- Playlists page: your playlists (collage covers, Synced / This device / Linked / Unfollowed chips),
  + makes one with its name ready to type, rename in place, sync / keep here, delete (asked), About
  items, sync conflicts (Choose…), Recently deleted; Made for you (hide).
- A playlist's page: rename by clicking the name, ⋯ and right-click on the whole header, reorder,
  ✕ (a followed file's songs grey in place), greyed missing / not here / linked entries with Add and
  Restore, conflicts. Covers cross-fade when they change.
- Linked playlists: Unfollowed (kept linked, not synced, greyed hidden; Follow the file again shows
  the differences again); a changed one deleted is kept 30 days in Recently deleted (restored as it
  was, over the file's) and its file in "Playlist files you let go of" (Follow again: the file's).
- All songs, Liked songs and Top songs pages (size on the page); All songs no longer opens Library.
- Virtual lists measure their place as they scroll (Library opened from another page drew wrong rows).
- Settings' playlists refresh by themselves; dragged greyed songs have a background.

## 0.5.7 (2026-10-18)

Library's Folders, and Play starting from the first song.

- Folders tab: a row per music folder (where it is, songs, "From the share", switched off greyed);
  a folder's page: the folder cover, Play and Shuffle, its songs by subfolder (drawn as on screen).
- Queue settings: "Play starts from the first song" (on): a list's Play turns shuffle off and starts
  at its first song, Shuffle turns it on; off: Play leaves shuffle as it is.
- No personal details in examples and tests (usernames, an IP address).

## 0.5.6 (2026-10-18)

Library's Albums and Artists, and their pages.

- Albums tab: album cards (sortable); right-click plays, shuffles, plays next, adds to the queue,
  goes to the artist. An album's page: big cover, "Album · artist · year · songs · length", Play
  and Shuffle, its songs with track numbers and disc headings (no covers or album column).
- Artists tab: a row per artist (sortable); an artist's page: name and line, Play and Shuffle,
  their Albums, Songs and Featured on. Shared pieces for the coming pages: ListHeader, LinkRow.
- Up next and Home's cards: Primary (white in dark mode, black in light mode) or like the cards.
- The player bar's queue button shows Now playing scrolled down to Up next at once.
- A cover without a picture shows the note; your name is out of the code comments.

## 0.5.5 (2026-10-18)

Library's Songs tab, and the library behind the coming pages.

- Songs carry their album's id, file path, date added and their artists with ids (one query), so
  every list links album and artist names; the separate artist lookup is gone.
- Sorting: songs (title, artist, album, date added, most played), albums (title, artist, year,
  date added), artists (name, most songs); album, artist and folder pages' data; Show in folder.
- The Songs tab: the whole library, drawing only the rows on screen (@tanstack/vue-virtual), a sort
  menu per tab (remembered), A–Z headings, plays / dates in the number column, playing all songs in
  the shown order ("songs:<sort>", still followed as All songs).
- The song menu works in every list (Play next, Add to queue, like, Go to album / artist, Show in
  folder), Now playing's More actions included; Add to playlist waits for the Playlists page.
- Now playing catches up with a queue the player resumed at start (it could show nothing playing).

## 0.5.4 (2026-10-18)

The app's frame, the player bar and Now playing.

- The frame: a custom title bar (Windows), one top bar for the main pages that stays put, the side
  dots, page switches that slide the way the dots go; pages opened on top (artist, album, folder,
  playlist, Search) are layers with their own top bar, sliding in from the right; vue-router; the
  settings page as a page (Sync & settings, a skeleton while it loads). Search: Ctrl Space or "/".
- The player bar: always there (empty: "Nothing playing", shuffle and repeat still on), the
  player's own icons, ✕ clears the player; hidden on Now playing.
- Now playing: the cover, the song (two-line title, artists and "Playing from" linking to their
  pages), volume, seek, controls, like (soft fill and a pop), an empty page when nothing plays.
- Up next: reordering with shuffle (moveInPlayOrder), shown at once and gliding into place, one
  highlight while placing, black highlight, ✕ and Clear fold away, playing a song moves it to the
  front (Previous goes back), with repeat all the next pass shown greyed in its real order.
- Cut-off text ends in ".." (a one-glyph font); a list started with shuffle on begins on a random song.
- Sync when coming back measures time away (desktop: the window's focus; Android: "paused").
- tauri dev also watches the native-audio plugin; native-audio v1.9.0.

## 0.5.3 (2026-10-17)

The song list and cover cards; the window shows first at start-up.

- Start-up: the window is shown as the first step (it starts hidden, for starting in the tray), so a slow start-up
  never leaves it hidden.
- Song list (components/songs): song rows with cover, title, artist, album, length, play, heart, menu (also
  right-click) and remove; the queue adds drag handles and numbers, reordering with the pointer (lifted, the others
  make room, numbers and "Plays next" follow), gliding into place. Highlight: white like a card in light mode, raised
  grey in dark. Playing bars that fall with gravity when paused. A–Z headings; local-only songs get the "This device"
  badge; missing songs greyed with a note icon. Menu: Play next, Add to queue, Add to liked / Remove from liked, Go to
  album / artist, Add to playlist.
- Cover cards (components/covers): picture, Cover maker gradient (bright colour at the bottom), collage, or auto
  playlist icon (Top songs cool to warm); the corner chip; the whole card highlights; right-click menus per kind.
- Fixes: segmented controls measure exactly (a fraction short wrapped the label copy out of sight); the right-click
  menu's red items use Melue's red.

## 0.5.2 (2026-10-17)

Settings page steps 4-6: Playback, Library, Display, Storage, About, the whole page.

- Playback: the queue following its list (full summaries, Customize, "Show me how it works" example), the end of
  the queue, audio settings (volume curve, media keys, previous/next, start-up, previous restarting, phone keep-alive),
  the output device (System default first, an unplugged device kept, chosen but grey). Melue keeps the player
  settings (player_settings.rs) and hands them to the player at start and on change.
- Library: the status card (counts, Scan now with progress, songs only here, copies), music folders (open to tick
  and move songs, a menu to switch off, rename, remove), share setup (address read back, errors under the field,
  More options, where synced music goes) and Your share, the phone's music source, playlists (Top songs size,
  hidden, recently deleted), Add songs and dropping files.
- Display: theme with Adaptive (dark and light by the clock), Start on (not wired yet); desktop: start with Windows
  (autostart plugin, starts in the tray), closing the window hides to the tray (tray icon, single instance).
  Storage: covers' size and Clean up. About: version, copy details for troubleshooting.
- The Settings page (a Settings tab in the spike), Components keeps the building blocks; new pieces: ConfirmDialog,
  Field, a partly-picked checkbox.
- The right-click menu: Reset to default with the default shown, About this setting, Copy link
  (melue://settings/<id>), This device only. Every setting in a registry with its default and help.
- Light mode: things on cards are white with a line (wells, off switches, chips, outline buttons).

## 0.5.1 (2026-10-17)

Settings page step 3, Syncing; reset to default menu.

- Syncing components: When it syncs (triggers), Songs (removing gone songs, asking first, moving new songs),
  On the share (folder layout with reorganise), Mobile data (levels wired; limits and usage not wired yet).
- Shared sync settings store, and the settings' defaults (matching Melue's Rust defaults).
- Right-click (long-press) on a setting row: Reset to default, greyed when it already is. The browser's own menu
  is off except in text fields.
- Radio mark: the ring is an inset shadow, so it animates smoothly.
- The design's comments and labels no longer name a person; the phones' usage says "Your phones".

## 0.5.0 (2026-10-17)

Phase 3 starts: Melue's design system, the spike's Components page, the General settings.

- Theme: shadcn's neutral palette with Melue's own surfaces (light: white cards on #ededed; dark: #171717 on #0a0a0a),
  Inter, badge / dot / "new" colours, 2 px separators; light or dark follows the system (lib/theme.ts).
- shadcn-vue components on reka-ui (with @vueuse/core), restyled where the design differs (switch, buttons, input).
- Settings building blocks (components/settings): card with the per-card trailing icon column, row and detail row,
  label, heading, tip, badge, tile, dot, segments (inverting slide), chips, dropdown value, stepper (sliding
  number), hint, radio, chevron, field row, Reveal; the motion standard (unfold, drop, swap, turning chevrons).
- The spike: Tests / Components switch; the Components page shows the finished components; a dev-only browser
  preview with sample data (?page=components&status=...), left out of the build.
- General settings, wired to Melue: the sync status card (every state, details, password, home address, quick syncs
  held on screen) and Across devices (mode, continue without asking, this device's name).

## 0.4.14 (2026-10-11)

Car crash fix, a log file on the phone, a short share wait in the car.

- native-audio 1.8.1: the player no longer gets killed when Android Auto starts it with Melue closed
  (Melue missing in Android Auto, playback stopping when the app closed).
- logging.rs: on Android, everything Melue prints goes to Android's log (tag "Melue") and to
  Android/data/com.simon.melue/files/melue.log (1 MB, then melue.log.1), kept for days.
- The car gives up connecting to the share after 5 s (setup::open_quick), not 15.

## 0.4.13 (2026-10-07)

The share's home address, looser same-song matching.

An optional home address (an address or a name), tried first for 2 s when this device's own address
is in its range (at home), else the main one; a note when it didn't answer at home. Same-song
matching ignores version notes in titles (Remastered, Radio Edit, Single/Album Version, Mono,
feat. X; whole words) and featured artists; the length check stays.

## 0.4.12 (2026-10-07)

Sync step 11: car likes and plays, linked playlists follow moved songs, the layout on the share.

Likes and plays in the car (Melue closed) are saved and shared right away (car.rs, CarQueue.kt).
Linked playlist entries follow songs moved to sync or hidden as copies; the linked matching takes a
hidden copy for its synced song. The share's layout lives in .melue/share.json, read at every sync,
so every device uses the same one. Spike: one Sync section; settings re-read before a change (an old
copy no longer undoes changes); the layout notice refreshes after library changes; on Android a
warning before moving to sync or clearing space.

## 0.4.11 (2026-10-07)

Sync step 10b: the share's layout, reorganising, the real move.

'Folders on the share': Artist/Album/ (default) or all in one folder; moving to sync follows it;
Reorganise now renames songs on the share and here (no downloads), updates .m3u files and Melue's
synced playlists, removes empty folders; once applied, new songs are put in place after a scan.
Moving to sync moves the file on the same drive (desktop) instead of copying.

## 0.4.10 (2026-10-07)

Sync step 10a: synced playback in the car with Melue closed.

Melue's queue provider (CarQueue.kt) calls Melue's Rust code through JNI (sync/car.rs): another
device's recent playback is loaded at start (the 10-minute rule; follow / off), 'Continue from
<device>' tops Recently played, and what the car plays is saved to the share. The playback core in
handoff.rs works without the app; the password helpers moved to MelueSecrets.kt. jni dependency
(Android).

## 0.4.9 (2026-10-07)

Sync step 9: moving songs to sync, the Not on your share list, hidden copies.

Move to sync: upload into Artist/Album/ (Uploads/ untagged; 'name (2)' when taken), copy into the
synced folder locally, then the original goes (Recycle Bin; Android's trash after its dialog;
deleted in a chosen folder) once the synced copy is in, its likes, plays and playlist entries
carried over. Local copies of synced songs are hidden; the user clears the space or keeps them.
'Move new songs to sync automatically' (off). Look for copies now (with near misses). Migration 12;
trash dependency (desktop).

## 0.4.8 (2026-10-07)

Sync step 8: likes, plays, history and playlists across devices.

Each device's likes, play counts (and per day) and latest 200 plays in .melue/library/<device>.json;
the newest like or unlike wins, counts add up (all_* views), the share's clock decides. Melue's
playlists in .melue/playlists/<id>.json, three-way merged from each device's last-synced version;
conflicts wait for a choice per playlist (mine / theirs / both); deleting deletes everywhere,
restorable for 30 days; 'Only on this device'; songs not here yet stay greyed. The same song
elsewhere (artist, title, length) counts as the share's. Migrations 10 and 11.

## 0.4.7 (2026-10-06)

Sync step 7: auto playlists, continuing without asking, reconnecting by itself.

Auto playlists (All songs, Liked, Top songs for the last month / 6 months / year / all time with sizes,
one per folder; hideable, shown again in the settings) from plays per day (migration 9), also in Android
Auto. Synced playback: continues without asking within 10 minutes (a setting), goes by the share's
clock. The share: 15 s to connect, retries after 30 s / 1 / 2 / 5 min, and on Android right away when
the network changes.

## 0.4.6 (2026-10-06)

Sync step 6: synced playback across devices.

Each device keeps what it plays in .melue/playback/<device>.json on the share; another device's newer
playback is offered (ask, follow or off), loaded paused, never interrupting playback. Change
notifications now say which file changed.

## 0.4.5 (2026-10-05)

Sync step 5: when it syncs.

- Syncing by itself (sync/schedule.rs rules, sync/scheduler.rs): at start-up,
  coming back to the app after 15 minutes (Android: opening it again;
  desktop: back into the window), when the share says its music changed (SMB
  change notifications, settled for 5 s; tested on the real NAS), and an
  optional periodic check (off by default). One sync at a time.
- Metered connections (mobile data, hotspots; a VPN on Wi-Fi isn't): song
  copies wait for Wi-Fi, moves/renames/removals still happen; a setting
  copies on mobile data (off).
- The sync status as a live sync-status event: progress, the last sync
  (kept across restarts), the last problem.
- Android: a "Syncing music" notification (a dataSync foreground service in
  Melue's plugin) with a progress bar and Cancel, which keeps long syncs
  going in the background.
- Fix: Melue's Android permission check asks Android directly first (Tauri's
  helper answered "denied" with the permission granted).
- Spike: the status line, the settings, every finished sync in the log, and
  a simulated sync notification.

## 0.4.4 (2026-10-05)

The version numbers (missed in the step's commit).

## 0.4.4 (2026-10-05)

Sync step 4: safe copying.

- Sync now carries out the plan: downloads and updates streamed into a hidden
  .melue-part file and put in place when complete; moves follow the share
  (Android: Melue's plugin moves the document, so it keeps its time); adopted
  files remembered; removals with the setting and the user's OK. Each file is
  remembered right after it's copied: an interrupted sync carries on; a
  failed update keeps the old copy.
- Free space checked first (plus 500 MB kept free; fs4).
- A bad file is skipped and reported; losing the share stops the sync; one
  sync at a time; progress events; Cancel stops after the current file.
- The library scans the synced folder afterwards.
- Android: the synced folder as a share (sync/tree.rs).
- The spike has Sync now, Cancel and the removal OK.

## 0.4.3 (2026-10-05)

Sync step 3: the sync plan (dry run).

- The share's music folder compared with the synced folder and with what was
  synced before (migration 8, synced_files): download (new, or deleted here),
  update (changed on the share, or changed here: undone), adopt (already here,
  same size, never synced), move (moved or renamed on the share: no download,
  no duplicate), gone from the share (kept, or removed with the setting),
  only here (put by hand: stays), forget.
- Only songs, playlist files and cover images; hidden folders (Melue's own
  .melue/) left out; paths compare without case.
- Settings: remove songs gone from the share (off), ask before removing more
  than 5 (0: always).
- A different share or folder, or removing it, forgets what was synced.
- The spike has a Dry run button and the two settings.

## 0.4.2 (2026-10-05)

Sync step 2: share setup and secure credentials.

- Share setup (share/setup.rs): an SMB share, or on desktop a network drive
  (copied, or played straight from); tested first, saved in the sync_share
  setting without the password; change password, remove (the synced folder
  stays as a normal folder with its songs).
- The password in a store of the user's choice: the system's secure store
  (keyring: Credential Manager, Keychain, Secret Service; Android Keystore
  through Melue's plugin), asked each time (memory only), or a plain file;
  without a secure store the setup offers the other two.
- The synced folder: Music/Melue on desktop, chosen with the picker on
  Android; a synced library folder that may be nested (the outer folder's
  scan skips it) and sits next to "all music on this phone" (which skips it).
  Android: show in / hide from other apps (.nomedia).
- Fixes: chosen Android folders now keep their access (the top tree address
  is what Android grants; broken since Phase 1's chosen folders); asking for
  a permission denied for good answers "denied" at once instead of waiting.
- The spike's test switch is replaced by a share setup block.

## 0.4.1 (2026-10-05)

Sync step 1: share access in Rust, plus a test kit.

- One share interface (src-tauri/src/share): list, walk, stat, streamed and
  whole reads, safe writes (a hidden .melue-part file first, put in place when
  complete), rename, delete, folders, free space. Paths stay inside the music
  folder.
- SMB on smb2, on a small runtime of its own, usable from any thread (Tauri's
  async ones too): 5 s to connect, 30 s per silent operation, one reconnect
  and retry before any data moved; the password never shows in logs.
- A folder version (tests; network drives on desktop later).
- Errors sorted for people: unreachable, login, not found, no permission,
  full, other.
- Test kit: a temporary share and a share that fails on purpose; an opt-in
  test against the real NAS (in its own temporary folder, removed after).
- The spike's SMB commands run on it; the old TypeScript SmbSource layer is
  gone.

## 0.3.12 (2026-10-05)

Library steps 12 and 13: sync available, a tidy spike section.

- sync.available (src/library/sync.ts): live, true when a share is set up
  (the sync_share setting, filled by Phase 2's share setup); screens show sync
  buttons only then. Changes arrive as library-changed with reason "sync".
  capabilities stays for what never changes while the app runs.
- The spike's library section is grouped (folders and scanning, browsing,
  playing, sync), with a test share switch until Phase 2; the desktop spike
  keeps the app's recently played lists.

Phase 1 (the library) is complete.

## 0.3.11 (2026-10-05)

Library step 11: the Android Auto library.

- Android Auto's library built from the database in Rust and sent through
  native-audio v1.5.0 at start-up and about 2 s after changes settle (all
  changes included), also with the UI asleep.
- Tabs: Recently played (the player's tracked list of lists played),
  Playlists (Liked songs first), Albums, Artists (one list of their songs).
  No All songs tab (search and voice find songs), no section headers.
- Folder ids are the list ids, so lists played in the car are followed by
  the queue; tapping a list opens it, a song in it plays the list from there.
- The spike's own Android Auto test library is gone.

## 0.3.10 (2026-10-04)

Library step 10: connecting the player.

- Playing lists from the library: songs, album, artist, playlist, liked
  (library ids, files, large covers in the queue).
- Playback events counted once each (also those from while the app was
  closed): a play is played to the end or left after half or 4 minutes, a
  skip is left in the first 30 s. History keeps the latest 10,000 starts;
  counts live in track_stats. Recently played and most played (migration 7).
- Likes, with the player's like button (buttons now: repeat, shuffle, like).
- The queue follows library changes to the list it plays, in Rust (through
  native-audio v1.4.0's Rust API), with settings (on by default).
- The spike page plays lists, likes songs and lists recent / most played.

## 0.3.9 (2026-10-04)

Library step 9: linked .m3u playlists.

- Scans turn .m3u/.m3u8 files in the music folders into linked playlists:
  lines matched by path, end of path (at least folder and name) or #EXTINF;
  unmatched lines stay as "not found" and match when the song turns up.
- Melue keeps its own version plus a snapshot of the file: entries are both,
  here_only, removed_here, suggested or file_removed. File changes follow the
  playlist's settings (new songs added or suggested, removed ones removed or
  kept); the file's order is taken only when it changed.
- Write-back (off by default) writes Melue's version in the file's own style,
  never half-written (Android chosen folders too).
- Gone files keep their playlist ("file gone"), renamed ones are followed,
  unlinking keeps it as an app playlist; let-go files aren't linked again
  (migration 6).
- The spike page shows states, settings, restore and unlink.

## 0.3.8 (2026-10-04)

Library step 8: app playlists.

- Playlists made in Melue: create, rename, delete; add songs (at a position),
  move and remove them. The same song can be in a playlist twice.
- Songs that are missing or in a switched-off folder stay, unavailable; songs
  that leave the library leave their playlists.
- A playlist's count, available count, length and first four covers.
- Playlist edits send library-changed (reason playlists, playlist:<id>); song
  changes name the playlists they're in.
- The spike page can make and edit playlists.

## 0.3.7 (2026-10-04)

Library step 7: library change event.

One library-changed event per change (a scan that found something, folder
actions) with the songs that changed and the playable lists whose songs
changed (songs, album:<id>, artist:<id>). Found by comparing snapshots of every
song from before and after, so no kind of change is missed (about 40 ms on
10,000 songs). The spike page refreshes its lists from it. Keeping the queue
in step with the library moves to step 9 (connecting the player).

## 0.3.3 (2026-10-04)

Library step 3: music folders.

- Music folders in the library: add, list, rename, switch on/off, remove; paths normalized (the same
  folder written differently is recognized), folders inside or around another refused; on Android all
  music on the phone and chosen folders are either/or
- Melue's own Android plugin (src-tauri/plugins/melue-android): the music permission (READ_MEDIA_AUDIO,
  storage on Android 12 and older) and the music index's song count
- Chosen Android folders keep their access across restarts (and give it back when removed)
- Spike: section 6 manages folders; the unconnected storage picker section is gone
- Versioning from here: 0.<phase + 2>.<step> until the first release (1.0.0), with a tag per step

## 0.2.1 (2026-10-03)

Renamed the app to Melue, added tracked lists, fixed song names after a fresh install.

- Renamed phoenix to Melue: app name, identifier com.simon.melue, package and
  crate names (melue, melue_lib), Android package and theme.
- The signing key is no longer tracked (*.jks ignored); keystore.properties finds
  it by a relative path.
- src/platform/trackedLists: tracked lists through the platform layer (native-audio
  on mobile, desktop later); the spike's recently played uses it.
- Spike: failed tag reads are no longer cached, so songs downloaded after the
  library was sent get their names.
- The native-audio plugin is linked from its repo root (root package.json).

## 0.2.0 (2026-10-02)

Added platform layer, desktop/mobile shells and new app icon.
