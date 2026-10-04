//! Linked playlists (linked.rs): playlist files in the music folders, read into the library, followed as
//! they change, and (opt-in) written back.

use super::linked::LocalFiles;
use super::testkit::{MusicFolder, Tags, TestLibrary};
use super::playlists::Playlist;

/// A library with one folder of songs A/01.wav ... A/0<n>.wav titled "Song 1" ... by "Artist".
fn setup(name: &str, songs: u32) -> (TestLibrary, MusicFolder, i64) {
    let t = TestLibrary::new(name);
    let m = t.music_folder("Music");
    let f = t.add_folder(&m).id;
    for n in 1..=songs {
        m.add(&format!("A/{n:02}.wav"), &Tags::song(&format!("Song {n}"), "Artist", "Album", n));
    }
    t.scan(f, &m);
    (t, m, f)
}

/// Writes a playlist file whose lines are these songs, relative to Lists/ ("../A/01.wav").
fn write_list(m: &MusicFolder, name: &str, songs: &[u32]) {
    let lines: String = songs.iter().map(|n| format!("../A/{n:02}.wav\n")).collect();
    rewrite(m, &format!("Lists/{name}"), &lines);
}

/// Rewrites a file so a scan sees it changed (a later modified time even within the same second).
fn rewrite(m: &MusicFolder, relative: &str, text: &str) {
    let path = m.path(relative);
    let before = std::fs::metadata(&path).ok().and_then(|meta| meta.modified().ok());
    m.add_text(relative, text);
    if let Some(before) = before {
        m.touch(relative, before + std::time::Duration::from_secs(2));
    }
}

fn playlist(t: &TestLibrary, name: &str) -> Playlist {
    t.library.playlists().unwrap().into_iter().find(|p| p.name == name).unwrap_or_else(|| panic!("no playlist {name}"))
}

/// The playlist's entries as (song number or the file line, state).
fn entries(t: &TestLibrary, id: i64) -> Vec<(String, String)> {
    t.library
        .playlist_entries(id)
        .unwrap()
        .into_iter()
        .map(|e| {
            let what = match &e.track {
                Some(track) => track.title.trim_start_matches("Song ").to_string(),
                None => format!("? {}", e.file_entry.unwrap_or_default()),
            };
            (what, e.state)
        })
        .collect()
}

fn list(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

fn entry_id(t: &TestLibrary, playlist: i64, song: u32) -> i64 {
    let title = format!("Song {song}");
    t.library.playlist_entries(playlist).unwrap().into_iter().find(|e| e.track.as_ref().is_some_and(|tr| tr.title == title)).unwrap().id
}

fn track(t: &TestLibrary, song: u32) -> i64 {
    t.library.conn().query_row("SELECT id FROM tracks WHERE path = ?1", [format!("A/{song:02}.wav")], |r| r.get(0)).unwrap()
}

#[test]
fn a_playlist_file_becomes_a_linked_playlist() {
    let (t, m, f) = setup("linked-new", 3);
    m.add_text("Lists/Road Trip.m3u8", "#EXTM3U\n#EXTINF:1,Artist - Song 2\n../A/02.wav\n../A/01.wav\nhttps://radio.example/live\n");
    t.scan(f, &m);
    let p = playlist(&t, "Road Trip");
    assert_eq!((p.kind.as_str(), p.file.as_deref(), p.file_missing), ("linked", Some("Road Trip.m3u8"), false));
    assert_eq!((p.new_in_file.as_str(), p.removed_from_file.as_str(), p.write_back), ("added", "removed", false), "the defaults");
    assert_eq!(entries(&t, p.id), list(&[("2", "both"), ("1", "both"), ("? https://radio.example/live", "both")]));
    assert_eq!((p.entries, p.available), (3, 2));
    // A scan with nothing changed reads nothing again.
    let updated = p.updated_at;
    t.scan(f, &m);
    assert_eq!(playlist(&t, "Road Trip").updated_at, updated);
}

#[test]
fn lines_are_matched_by_path_end_of_path_or_artist_and_title() {
    let (t, m, f) = setup("linked-matching", 5);
    m.add("B/01.wav", &Tags::song("Other one", "Someone", "B", 1)); // a second "01.wav"
    t.scan(f, &m);
    let exact = m.path("A/05.wav").to_string_lossy().into_owned();
    let text = format!(
        "D:\\Old PC\\Music\\A\\03.wav\n/home/me/Music/A/04.wav\n#EXTINF:1,Artist - Song 2\nlost/elsewhere.mp3\n{exact}\nX/01.wav\nnothing.mp3\nfile:///C:/Somewhere/A/01.wav\n"
    );
    m.add_text("Lists/Mixed.m3u", &text);
    t.scan(f, &m);
    let p = playlist(&t, "Mixed");
    assert_eq!(
        entries(&t, p.id),
        list(&[("3", "both"), ("4", "both"), ("2", "both"), ("5", "both"), ("? X/01.wav", "both"), ("? nothing.mp3", "both"), ("1", "both")])
    );
}

#[test]
fn changes_in_the_file_with_the_default_settings() {
    let (t, m, f) = setup("linked-defaults", 5);
    write_list(&m, "Mix.m3u", &[1, 2, 3]);
    t.scan(f, &m);
    let p = playlist(&t, "Mix").id;
    // In Melue: 2 removed (greyed: the file still has it), 4 added (only here).
    t.library.remove_from_playlist(p, &[entry_id(&t, p, 2)]).unwrap();
    t.library.add_to_playlist(p, &[track(&t, 4)], None).unwrap();
    assert_eq!(entries(&t, p), list(&[("1", "both"), ("2", "removed_here"), ("3", "both"), ("4", "here_only")]));

    // In the file: 2 removed, 5 added. Same order otherwise, so Melue's order stays; 5 is last in the file,
    // so it's last here too. 6 put between 1 and 3 goes before 3.
    write_list(&m, "Mix.m3u", &[1, 3, 5]);
    t.scan(f, &m);
    assert_eq!(entries(&t, p), list(&[("1", "both"), ("3", "both"), ("4", "here_only"), ("5", "both")]));
    m.add("A/06.wav", &Tags::song("Song 6", "Artist", "Album", 6));
    write_list(&m, "Mix.m3u", &[1, 6, 3, 5]);
    t.scan(f, &m);
    assert_eq!(entries(&t, p), list(&[("1", "both"), ("6", "both"), ("3", "both"), ("4", "here_only"), ("5", "both")]));
}

#[test]
fn changes_in_the_file_suggested_and_kept() {
    let (t, m, f) = setup("linked-suggested", 6);
    write_list(&m, "Mix.m3u", &[1, 3, 5]);
    t.scan(f, &m);
    let p = playlist(&t, "Mix").id;
    t.library.add_to_playlist(p, &[track(&t, 4)], None).unwrap();
    t.library.set_playlist_options(p, Some("suggested"), Some("kept"), None).unwrap();
    assert!(t.library.set_playlist_options(p, Some("maybe"), None, None).is_err());

    // The file reordered, 5 removed, 6 new: the file's order, 5 kept (marked), 6 suggested, and Melue's
    // 4 after the song it followed (5, which stays after 3).
    write_list(&m, "Mix.m3u", &[3, 1, 6]);
    t.scan(f, &m);
    assert_eq!(
        entries(&t, p),
        list(&[("3", "both"), ("5", "file_removed"), ("4", "here_only"), ("1", "both"), ("6", "suggested")])
    );
    let mix = playlist(&t, "Mix");
    assert_eq!((mix.entries, mix.suggested), (4, 1), "a suggestion isn't one of its songs yet");

    // Taking the suggestion; turning one down (it stays greyed: the file has it).
    t.library.restore_playlist_entry(p, entry_id(&t, p, 6)).unwrap();
    t.library.remove_from_playlist(p, &[entry_id(&t, p, 1)]).unwrap();
    assert_eq!(entries(&t, p)[3], ("1".into(), "removed_here".into()));
    t.library.restore_playlist_entry(p, entry_id(&t, p, 1)).unwrap();
    assert!(t.library.restore_playlist_entry(p, entry_id(&t, p, 3)).is_err(), "only greyed ones");

    // 5 back in the file: the kept entry is "both" again, not a second copy.
    write_list(&m, "Mix.m3u", &[3, 1, 6, 5]);
    t.scan(f, &m);
    assert_eq!(entries(&t, p).iter().filter(|(song, _)| song == "5").count(), 1);
}

#[test]
fn melues_order_stays_while_the_file_keeps_its_order() {
    let (t, m, f) = setup("linked-order", 4);
    write_list(&m, "Mix.m3u", &[1, 2, 3]);
    t.scan(f, &m);
    let p = playlist(&t, "Mix").id;
    t.library.move_in_playlist(p, entry_id(&t, p, 3), 0).unwrap();
    write_list(&m, "Mix.m3u", &[1, 2, 3, 4]);
    t.scan(f, &m);
    assert_eq!(entries(&t, p), list(&[("3", "both"), ("1", "both"), ("2", "both"), ("4", "both")]));
}

#[test]
fn a_song_in_twice_loses_the_copy_removed_in_melue() {
    let (t, m, f) = setup("linked-twice", 2);
    write_list(&m, "Mix.m3u", &[1, 2, 1]);
    t.scan(f, &m);
    let p = playlist(&t, "Mix").id;
    let second = t.library.playlist_entries(p).unwrap()[2].id;
    t.library.remove_from_playlist(p, &[second]).unwrap();
    write_list(&m, "Mix.m3u", &[1, 2]);
    t.scan(f, &m);
    assert_eq!(entries(&t, p), list(&[("1", "both"), ("2", "both")]));
}

#[test]
fn writing_back_keeps_the_files_style() {
    let (t, m, f) = setup("linked-write", 3);
    m.add_text("Lists/Mix.m3u", "#EXTM3U\r\n#EXTINF:5,Old - Info\r\n..\\A\\01.wav\r\n..\\A\\02.wav\r\n");
    t.scan(f, &m);
    let p = playlist(&t, "Mix").id;
    t.library.remove_from_playlist(p, &[entry_id(&t, p, 2)]).unwrap();
    t.library.add_to_playlist(p, &[track(&t, 3)], Some(0)).unwrap();
    // Off by default: the file is untouched.
    assert!(!t.library.write_back_if_on(p, &LocalFiles).unwrap());
    t.library.set_playlist_options(p, None, None, Some(true)).unwrap();
    assert!(t.library.write_back_if_on(p, &LocalFiles).unwrap());

    let written = std::fs::read_to_string(m.path("Lists/Mix.m3u")).unwrap();
    assert_eq!(written, "#EXTM3U\r\n#EXTINF:0,Artist - Song 3\r\n..\\A\\03.wav\r\n#EXTINF:0,Artist - Song 1\r\n..\\A\\01.wav\r\n");
    assert_eq!(entries(&t, p), list(&[("3", "both"), ("1", "both")]));
    // The next scan finds nothing new in it.
    let updated = playlist(&t, "Mix").updated_at;
    t.scan(f, &m);
    assert_eq!(playlist(&t, "Mix").updated_at, updated);
}

#[test]
fn a_gone_file_keeps_its_playlist_and_a_renamed_one_is_followed() {
    let (t, m, f) = setup("linked-gone", 2);
    write_list(&m, "Mix.m3u", &[1, 2]);
    t.scan(f, &m);
    let p = playlist(&t, "Mix").id;

    m.rename("Lists/Mix.m3u", "Lists/Renamed.m3u");
    t.scan(f, &m);
    let mix = playlist(&t, "Mix");
    assert_eq!((mix.id, mix.file.as_deref(), mix.file_missing), (p, Some("Renamed.m3u"), false));

    m.remove("Lists/Renamed.m3u");
    t.scan(f, &m);
    let mix = playlist(&t, "Mix");
    assert!(mix.file_missing);
    assert_eq!(entries(&t, p).len(), 2, "its songs stay");
    assert!(t.library.set_playlist_options(p, None, None, Some(true)).is_ok());
    assert!(t.library.write_back(p, &LocalFiles).is_err(), "no file to write");

    // Kept as an app playlist.
    t.library.unlink_playlist(p).unwrap();
    let mix = playlist(&t, "Mix");
    assert_eq!((mix.kind.as_str(), mix.file, mix.file_missing), ("app", None, false));
    assert_eq!(entries(&t, p), list(&[("1", "both"), ("2", "both")]));
}

#[test]
fn unlinking_drops_the_greyed_songs() {
    let (t, m, f) = setup("linked-unlink", 3);
    write_list(&m, "Mix.m3u", &[1, 2]);
    t.scan(f, &m);
    let p = playlist(&t, "Mix").id;
    t.library.remove_from_playlist(p, &[entry_id(&t, p, 1)]).unwrap();
    t.library.add_to_playlist(p, &[track(&t, 3)], None).unwrap();
    t.library.unlink_playlist(p).unwrap();
    assert_eq!(entries(&t, p), list(&[("2", "both"), ("3", "both")]));
    // The file is still there, and isn't linked again (not even when it changes).
    write_list(&m, "Mix.m3u", &[1, 2, 3]);
    t.scan(f, &m);
    assert_eq!(t.library.playlists().unwrap().len(), 1);
}

#[test]
fn a_deleted_linked_playlist_isnt_linked_again() {
    let (t, m, f) = setup("linked-delete", 1);
    write_list(&m, "Mix.m3u", &[1]);
    t.scan(f, &m);
    t.library.delete_playlist(playlist(&t, "Mix").id).unwrap();
    t.scan(f, &m);
    assert!(t.library.playlists().unwrap().is_empty());
    assert!(m.path("Lists/Mix.m3u").exists(), "the file is left alone");
}

#[test]
fn songs_leaving_the_library_stay_as_lines_and_match_again() {
    let (t, m, f) = setup("linked-rematch", 1);
    let other = t.music_folder("Other");
    other.add("B/01.wav", &Tags::song("Guest song", "Guest", "B", 1));
    let g = t.add_folder(&other).id;
    t.scan(g, &other);
    let line = other.path("B/01.wav").to_string_lossy().into_owned();
    m.add_text("Lists/Mix.m3u", &format!("../A/01.wav\n{line}\n"));
    t.scan(f, &m);
    let p = playlist(&t, "Mix").id;
    assert_eq!(entries(&t, p).len(), 2);

    // Its folder removed: the line stays, not found...
    t.library.remove_folder(g).unwrap();
    t.scan(f, &m);
    assert_eq!(entries(&t, p), list(&[("1", "both"), (&format!("? {line}"), "both")]));
    // ...and finds its song when the folder is back.
    let g = t.add_folder(&other).id;
    t.scan(g, &other);
    assert_eq!(entries(&t, p)[1].0, "Guest song");
}

#[test]
fn a_removed_folder_leaves_its_playlists_with_their_file_gone() {
    let (t, m, f) = setup("linked-folder", 1);
    let lists = t.music_folder("Lists");
    let g = t.add_folder(&lists).id;
    let line = m.path("A/01.wav").to_string_lossy().into_owned();
    lists.add_text("Mix.m3u", &format!("{line}\n"));
    t.scan(g, &lists);
    let p = playlist(&t, "Mix").id;
    t.library.remove_folder(g).unwrap();
    t.scan(f, &m);
    let mix = playlist(&t, "Mix");
    assert_eq!((mix.id, mix.file_missing, mix.entries), (p, true, 1));
}
