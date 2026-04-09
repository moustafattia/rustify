use std::env;
use std::io::{self, BufRead};
use std::path::Path;

use rustify_core::player::{Player, PlayerConfig};
use rustify_core::types::path_to_uri;
use rustify_core::{playlist, scanner};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: play <file_or_dir> [--playlist] [--scan]");
        eprintln!("  play song.mp3           Play a single file");
        eprintln!("  play --scan /Music      Scan directory, play all");
        eprintln!("  play --playlist mix.m3u  Load M3U playlist");
        std::process::exit(1);
    }

    let config = PlayerConfig {
        alsa_device: "default".to_string(),
        music_dirs: vec![],
    };

    let player = Player::new(config).expect("Failed to create player");

    // Register display callbacks
    player.on_state_change(Box::new(|state| {
        println!("[State] {state:?}");
    }));
    player.on_track_change(Box::new(|track| {
        let artist = if track.artists.is_empty() {
            "Unknown".to_string()
        } else {
            track.artists.join(", ")
        };
        println!("[Track] {artist} — {}", track.name);
    }));
    player.on_position_update(Box::new(|ms| {
        let secs = ms / 1000;
        let mins = secs / 60;
        print!("\r[{:02}:{:02}]", mins, secs % 60);
    }));
    player.on_error(Box::new(|msg| {
        eprintln!("[Error] {msg}");
    }));

    // Parse args and load tracks
    let is_scan = args.contains(&"--scan".to_string());
    let is_playlist = args.contains(&"--playlist".to_string());
    let path_arg = args
        .iter()
        .find(|a| !a.starts_with('-') && *a != &args[0])
        .expect("No path provided");

    if is_scan {
        let uris = scanner::scan_directory(Path::new(path_arg)).expect("Scan failed");
        println!("Found {} tracks", uris.len());
        player.load_track_uris(uris);
    } else if is_playlist {
        let uris = playlist::parse_m3u(Path::new(path_arg)).expect("Playlist parse failed");
        println!("Loaded {} tracks from playlist", uris.len());
        player.load_track_uris(uris);
    } else {
        player.load_track_uris(vec![path_to_uri(Path::new(path_arg))]);
    }

    player.play();

    println!("Commands: play, pause, stop, next, prev, vol <0-100>, quit");
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        match line.trim() {
            "play" | "p" => player.play(),
            "pause" => player.pause(),
            "stop" | "s" => player.stop(),
            "next" | "n" => player.next(),
            "prev" => player.previous(),
            "quit" | "q" => {
                player.shutdown();
                break;
            }
            cmd if cmd.starts_with("vol ") => {
                if let Ok(vol) = cmd[4..].parse::<u8>() {
                    player.set_volume(vol);
                    println!("Volume: {vol}");
                }
            }
            "" => {}
            other => println!("Unknown command: {other}"),
        }
    }
}
