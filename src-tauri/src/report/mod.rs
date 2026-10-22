//! Bug reports (src/lib/report/): made on any device, with what's personal replaced first (scrub.rs); sent as public
//! issues on Melue's GitHub from the account signed in (github.rs), now or later: a report that can't go yet is kept
//! here, synced to the share and sent by whichever linked device is signed in (saved.rs). A screenshot, when the
//! person keeps them (off by default), stays on the device and the share, never sent (screenshot.rs).

pub mod saved;
pub mod scrub;
pub mod screenshot;
