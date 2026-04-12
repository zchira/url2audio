use std::thread::sleep;

use url2audio::player_engine::PlayerStatus;
use url2audio::Player;

fn main() {
    let src = "https://podcast.daskoimladja.com/media/2024-05-27-PONEDELJAK_27.05.2024.mp3";
    // let src = "https://stream.daskoimladja.com:9000/stream";

    let mut p = Player::new();

    let events = p.events().clone();
    std::thread::spawn(move || {
        while let Ok(event) = events.recv() {
            match event {
                PlayerStatus::Opened(ref src) => println!("[event] Opened: {}", src),
                PlayerStatus::Closed => println!("[event] Closed"),
                PlayerStatus::Seeked(pos) => println!("[event] Seeked to: {:.1}s", pos),
                PlayerStatus::SendPlaying(ref state) => println!("[event] Playing state: {:?}", state),
                PlayerStatus::Error(ref err) => println!("[event] Error: {}", err),
                _ => {}
            }
        }
    });

    p.open(src);

    sleep(std::time::Duration::from_secs(5));

    println!("{:#?}", p.buffer_chunks());

    println!("seeking to 120s...");
    p.seek(120.0);
    sleep(std::time::Duration::from_millis(5000));

    println!("seeking to 180s...");
    p.seek(180.0);
    sleep(std::time::Duration::from_millis(5000));

    println!("seeking to 150s...");
    p.seek(150.0);
    println!("sleep 8s...");
    sleep(std::time::Duration::from_secs(8));

    println!("close");
    p.close();
    sleep(std::time::Duration::from_secs(5));
    println!("closed. end.");
}
