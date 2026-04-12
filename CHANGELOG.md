# Changelog

## 0.4.0

### Performance improvements

- Replaced poll loop (`try_recv()` + `sleep(10ms)`) with blocking `recv()` in status thread (`lib.rs`)
- Replaced poll loop (`sleep(200ms)`) with blocking `recv()` in engine idle states (error, no reader, paused) (`player_engine.rs`)
- Removed redundant `sleep(20ms)` at the end of the play loop — CPAL audio output already applies backpressure
- Added `handle_action()` helper method and `ActionResult` enum for centralized action handling in the engine loop
- Removed unnecessary `sleep`/`Duration` imports from both files
