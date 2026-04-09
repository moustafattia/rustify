use std::collections::BTreeMap;

use rustify_core::types::Track;

/// An album in the library index.
#[derive(Debug, Clone)]
pub struct Album {
    pub name: String,
    pub artist: String,
    pub tracks: Vec<Track>,
}

/// In-memory music library index, organized by artist and album.
#[derive(Debug)]
pub struct Library {
    /// Artists sorted alphabetically, each with their albums.
    artists: BTreeMap<String, Vec<Album>>,
    /// Flat list of all tracks for the Songs view.
    tracks: Vec<Track>,
}

impl Library {
    /// Build a library index from a flat list of tracks.
    /// Groups by artist -> album, sorts tracks within albums by track number.
    pub fn from_tracks(tracks: Vec<Track>) -> Self {
        let mut artist_albums: BTreeMap<String, BTreeMap<String, Vec<Track>>> = BTreeMap::new();

        for track in &tracks {
            let artist_name = if track.artists.is_empty() {
                "Unknown Artist".to_string()
            } else {
                track.artists[0].clone()
            };

            artist_albums
                .entry(artist_name)
                .or_default()
                .entry(track.album.clone())
                .or_default()
                .push(track.clone());
        }

        let artists: BTreeMap<String, Vec<Album>> = artist_albums
            .into_iter()
            .map(|(artist_name, albums_map)| {
                let mut albums: Vec<Album> = albums_map
                    .into_iter()
                    .map(|(album_name, mut album_tracks)| {
                        album_tracks.sort_by_key(|t| t.track_no.unwrap_or(u32::MAX));
                        Album {
                            name: album_name,
                            artist: artist_name.clone(),
                            tracks: album_tracks,
                        }
                    })
                    .collect();
                albums.sort_by(|a, b| a.name.cmp(&b.name));
                (artist_name, albums)
            })
            .collect();

        let mut sorted_tracks = tracks;
        sorted_tracks.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        Self {
            artists,
            tracks: sorted_tracks,
        }
    }

    /// Sorted list of artist names.
    pub fn artist_names(&self) -> Vec<&String> {
        self.artists.keys().collect()
    }

    /// Albums for a given artist name.
    pub fn albums_by_artist(&self, artist: &str) -> &[Album] {
        self.artists.get(artist).map(Vec::as_slice).unwrap_or(&[])
    }

    /// All albums across all artists (sorted by name).
    pub fn all_albums(&self) -> Vec<&Album> {
        let mut albums: Vec<&Album> = self.artists.values().flat_map(|a| a.iter()).collect();
        albums.sort_by(|a, b| a.name.cmp(&b.name));
        albums
    }

    /// All tracks (sorted by name).
    pub fn all_tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Case-insensitive substring search across track names, artist names, and album names.
    /// Fuzzy search across track names, artist names, and album names.
    /// Returns results ranked by match quality (best first), capped at 50.
    pub fn fuzzy_search(&self, query: &str) -> Vec<SearchResult<'_>> {
        if query.is_empty() {
            return Vec::new();
        }

        use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
        use nucleo_matcher::{Config, Matcher, Utf32Str};

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut results: Vec<SearchResult<'_>> = self
            .tracks
            .iter()
            .filter_map(|track| {
                let haystack = format!(
                    "{} {} {}",
                    track.name,
                    track.artists.first().unwrap_or(&String::new()),
                    track.album
                );
                let mut buf = Vec::new();
                let utf32 = Utf32Str::new(&haystack, &mut buf);
                let score = pattern.score(utf32, &mut matcher)?;
                let mut indices = Vec::new();
                pattern.indices(utf32, &mut matcher, &mut indices);
                Some(SearchResult {
                    track,
                    score,
                    matched_indices: indices,
                })
            })
            .collect();

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(50);
        results
    }
}

/// A fuzzy search result with match score and highlighted character indices.
#[allow(dead_code)]
pub struct SearchResult<'a> {
    pub track: &'a Track,
    pub score: u32,
    pub matched_indices: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tracks() -> Vec<Track> {
        vec![
            Track {
                uri: "file:///music/m83/hurry/midnight.mp3".into(),
                name: "Midnight City".into(),
                artists: vec!["M83".into()],
                album: "Hurry Up, We're Dreaming".into(),
                length: 243_000,
                track_no: Some(1),
            },
            Track {
                uri: "file:///music/m83/hurry/reunion.mp3".into(),
                name: "Reunion".into(),
                artists: vec!["M83".into()],
                album: "Hurry Up, We're Dreaming".into(),
                length: 407_000,
                track_no: Some(2),
            },
            Track {
                uri: "file:///music/m83/saturdays/kim.mp3".into(),
                name: "Kim & Jessie".into(),
                artists: vec!["M83".into()],
                album: "Saturdays = Youth".into(),
                length: 315_000,
                track_no: Some(1),
            },
            Track {
                uri: "file:///music/radiohead/ok/paranoid.mp3".into(),
                name: "Paranoid Android".into(),
                artists: vec!["Radiohead".into()],
                album: "OK Computer".into(),
                length: 383_000,
                track_no: Some(2),
            },
        ]
    }

    #[test]
    fn build_library_groups_by_artist() {
        let lib = Library::from_tracks(make_tracks());
        let names = lib.artist_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&&"M83".to_string()));
        assert!(names.contains(&&"Radiohead".to_string()));
    }

    #[test]
    fn artist_albums_returns_correct_albums() {
        let lib = Library::from_tracks(make_tracks());
        let albums = lib.albums_by_artist("M83");
        assert_eq!(albums.len(), 2);
        let album_names: Vec<&str> = albums.iter().map(|a| a.name.as_str()).collect();
        assert!(album_names.contains(&"Hurry Up, We're Dreaming"));
        assert!(album_names.contains(&"Saturdays = Youth"));
    }

    #[test]
    fn album_tracks_returns_sorted_tracks() {
        let lib = Library::from_tracks(make_tracks());
        let albums = lib.albums_by_artist("M83");
        let hurry = albums.iter().find(|a| a.name.contains("Hurry")).unwrap();
        assert_eq!(hurry.tracks.len(), 2);
        assert_eq!(hurry.tracks[0].name, "Midnight City");
        assert_eq!(hurry.tracks[1].name, "Reunion");
    }

    #[test]
    fn all_tracks_returns_everything() {
        let lib = Library::from_tracks(make_tracks());
        assert_eq!(lib.all_tracks().len(), 4);
    }

    #[test]
    fn all_albums_returns_everything() {
        let lib = Library::from_tracks(make_tracks());
        assert_eq!(lib.all_albums().len(), 3);
    }

    #[test]
    fn empty_library() {
        let lib = Library::from_tracks(vec![]);
        assert!(lib.artist_names().is_empty());
        assert!(lib.all_tracks().is_empty());
        assert!(lib.all_albums().is_empty());
    }

    #[test]
    fn fuzzy_search_finds_exact_match() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.fuzzy_search("Midnight City");
        assert!(!results.is_empty());
        assert_eq!(results[0].track.name, "Midnight City");
    }

    #[test]
    fn fuzzy_search_partial_match() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.fuzzy_search("mid cit");
        assert!(!results.is_empty());
        assert_eq!(results[0].track.name, "Midnight City");
    }

    #[test]
    fn fuzzy_search_is_case_insensitive() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.fuzzy_search("PARANOID");
        assert!(!results.is_empty());
    }

    #[test]
    fn fuzzy_search_matches_artist_names() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.fuzzy_search("radiohead");
        assert!(!results.is_empty());
    }

    #[test]
    fn fuzzy_search_empty_query_returns_empty() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.fuzzy_search("");
        assert!(results.is_empty());
    }

    #[test]
    fn fuzzy_search_no_match() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.fuzzy_search("zzzzzzzzz");
        assert!(results.is_empty());
    }

    #[test]
    fn fuzzy_search_returns_matched_indices() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.fuzzy_search("Midnight");
        assert!(!results.is_empty());
        assert!(!results[0].matched_indices.is_empty());
    }
}
