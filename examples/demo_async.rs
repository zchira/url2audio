use std::thread::sleep;

use url2audio::player_async::PlayerAsync;

#[tokio::main]
async fn main() {
    let src = "https://podcast.daskoimladja.com/media/2024-05-27-PONEDELJAK_27.05.2024.mp3";

    let mut p = PlayerAsync::new();
    println!("Opening...");
    let _ = p.open(src).await;
    println!("[Done]");

    sleep(std::time::Duration::from_secs(2));

    println!("Seeking...2min...");
    p.seek(120.0).await;
    println!("[Done]");

    sleep(std::time::Duration::from_secs(3));

    println!("Seeking...50min...");
    p.seek(60.0 * 50.0).await;
    println!("[Done]");

    sleep(std::time::Duration::from_secs(10));
    println!("close");
    p.close();
    sleep(std::time::Duration::from_secs(5));
    println!("closed. end.");

}
