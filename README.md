[![Crate Badge]][Crate]
[![Docs Badge]][Docs]

[Crate Badge]: https://img.shields.io/crates/v/url2audio?logo=rust&style=flat-square
[Crate]: https://crates.io/crates/url2audio
[Docs Badge]: https://img.shields.io/docsrs/url2audio?logo=rust&style=flat-square
[Docs]: https://docs.rs/url2audio/


# url2audio

Simple to use rust library for playing audio streams.

# How to use?

```
// create Player instance 
let mut p = Player::new();

// open audio stream from url:
// example: https://something.from.the.web/xyz.mpr
let res = p.open(src);

println!("duration: {}", p.duration());
sleep(std::time::Duration::from_secs(3));

// pause playback
p.pause();

sleep(std::time::Duration::from_secs(3));
// resume playback
p.play();
println!("Resume at: {}", p.current_position());

sleep(std::time::Duration::from_secs(3));
// seek
p.seek(600.0);

sleep(std::time::Duration::from_secs(5));
```
