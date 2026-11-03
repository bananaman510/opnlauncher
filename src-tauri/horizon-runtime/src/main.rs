//! OpnLauncher builds Horizon's upstream native client as its managed game process.
//! Keep the allocator and mod API marker used by Horizon's own executable.

#[cfg(not(feature = "system-allocator"))]
#[global_allocator]
static ALLOCATOR: horizon_threads::Allocator = horizon_threads::Allocator;

static MOD_API_LEVEL: [u8; 27] = horizon_mods::api_level::MARK;

fn main() {
    std::hint::black_box(&MOD_API_LEVEL);
    horizon_client::entry::main();
}
