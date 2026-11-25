//! The software renderer draws on several threads by default outside wasm (#123).
#![cfg(not(target_arch = "wasm32"))]

#[test]
fn renders_on_several_threads_by_default() {
    let cores = std::thread::available_parallelism().map_or(1, usize::from);

    // vello_cpu keeps one core for the caller, so a single-core machine
    // legitimately renders on the calling thread alone.
    if cores < 2 {
        return;
    }

    let settings = vello_cpu::RenderSettings::default();

    assert!(
        settings.num_threads > 0,
        "vello_cpu renders on one thread; its `multithreading` feature is off \
         (machine has {cores} cores)"
    );
}
