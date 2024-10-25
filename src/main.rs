use std::thread::sleep;

use url2audio::Player;

mod cpalaudio;
pub mod player_engine;
mod resampler;
pub mod url_source;
pub mod url_source_buff;

fn main() {
    let src = "https://podcast.daskoimladja.com/media/2024-05-27-PONEDELJAK_27.05.2024.mp3";
    // let src = "https://stream.daskoimladja.com:9000/stream";

    let mut p = Player::new();
    let res = p.open(src);

    sleep(std::time::Duration::from_secs(5));

    println!("{:#?}", p.buffer_chunks());
    p.seek(120.0);
    sleep(std::time::Duration::from_millis(200));
    p.seek(180.0);
    sleep(std::time::Duration::from_millis(200));
    p.seek(150.0);
    sleep(std::time::Duration::from_secs(20));

    // println!("Res: {:#?}", res);
    //
    // println!("duration: {}, {}", p.duration(), p.duration_display());
    // sleep(std::time::Duration::from_secs(20));
    // println!("Paused at: {}  {}", p.current_position_display(), p.current_position());
    // println!("duration: {} {}", p.duration(), p.duration_display());
    // p.pause();
    //
    // sleep(std::time::Duration::from_secs(3));
    // p.play();
    // println!("Resume at: {} {}", p.current_position_display(), p.current_position());
    //
    // sleep(std::time::Duration::from_secs(3));
    // p.seek(600.0);
    // println!("seek 600: {} {}", p.current_position_display(), p.current_position());
    //
    // sleep(std::time::Duration::from_secs(5));
    // p.seek(1200.0);
    // println!("seek 1200: {} {}", p.current_position_display(), p.current_position());
    //
    // sleep(std::time::Duration::from_secs(5));
    // p.seek(0.0);
    // println!("seek back 0: {} {}", p.current_position_display(), p.current_position());
    //
    // sleep(std::time::Duration::from_secs(5));
    // let res = p.open(src);
    // p.seek(600.0);
    // println!("open again: {:#?}", res);
    // sleep(std::time::Duration::from_secs(5));

    println!("close");
    p.close();
    sleep(std::time::Duration::from_secs(5));
    println!("closed. end.");

}
