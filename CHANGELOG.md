# Changelog

## 0.4.0

### Action feedback events

- Added `PlayerStatus::Opened(String)`, `PlayerStatus::Closed`, `PlayerStatus::Seeked(f64)` variants for action completion feedback
- Added `Player::events()` — returns `&Receiver<PlayerStatus>` for streaming player events (`Opened`, `Closed`, `Seeked`, `SendPlaying`, `Error`)
- Engine now sends `Opened` after successful open, `Closed` before closing, and `Seeked` after successful seek

### Performance improvements

- Replaced poll loop (`try_recv()` + `sleep(10ms)`) with blocking `recv()` in status thread (`lib.rs`)
- Replaced poll loop (`sleep(200ms)`) with blocking `recv()` in engine idle states (error, no reader, paused) (`player_engine.rs`)
- Removed redundant `sleep(20ms)` at the end of the play loop — CPAL audio output already applies backpressure
- Added `handle_action()` helper method and `ActionResult` enum for centralized action handling in the engine loop
- Removed unnecessary `sleep`/`Duration` imports from both files
- Added chunk eviction in `UrlSourceBuf` — only ±32 chunks (~2MB) around current position are kept in memory, distant chunks are evicted and re-fetched via HTTP Range on seek
