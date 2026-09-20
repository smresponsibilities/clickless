//! Replay trace for the dense-grid sequence (Prompt 2 harness).
//!
//! Drives the real state machine and rasterizer headless and prints one
//! tagged record per transition: key/phase, engine layer, overlay level,
//! cell counts split by one- and two-character labels, pointer, render
//! result and render time. Run it beside a native session and compare
//! records when labels flicker; a divergence localizes the fault to state,
//! frame composition or presentation.

use clickless_backend_api::overlay::render_frame;
use clickless_core::grid::GridConfig;
use clickless_core::{KeyEvent, LogicalKey, Phase, StateMachine};
use std::time::Instant;

fn step(tag: &str, key: &str, event: Option<(LogicalKey, Phase)>, sm: &mut StateMachine) {
    if let Some((k, p)) = event {
        sm.on_event(KeyEvent::new(k, p), 0);
    }
    let frame = sm.grid_overlay();
    let start = Instant::now();
    let rendered = frame.as_ref().map(|f| render_frame(f).is_some());
    let render_ms = start.elapsed().as_millis();
    match frame {
        Some(f) => {
            let two = f
                .cells
                .iter()
                .filter(|c| c.label.chars().count() == 2)
                .count();
            let one = f
                .cells
                .iter()
                .filter(|c| c.label.chars().count() == 1)
                .count();
            println!(
                "{tag:<15}\tkey={key:<10}\tlayer={:?}\tlevel={}\ttwo={two}\tone={one}\tpointer={:?}\trendered={rendered:?}\trender_ms={render_ms}",
                sm.layer(),
                f.level,
                f.pointer
            );
        }
        None => println!(
            "{tag:<15}\tkey={key:<10}\tlayer={:?}\tlevel=-\ttwo=0\tone=0\tpointer=-\trendered={rendered:?}\trender_ms={render_ms}",
            sm.layer()
        ),
    }
}

fn run(name: &str, width: i64, height: i64) {
    println!("== dense {name} {width}x{height} ==");
    let mut sm = StateMachine::new();
    sm.enable_grid(width, height, GridConfig::dense());

    step(
        "leader-press",
        "caps",
        Some((LogicalKey::CapsLock, Phase::Press)),
        &mut sm,
    );
    sm.poll(200); // cross the hold threshold
    step(
        "activate",
        "space",
        Some((LogicalKey::Space, Phase::Press)),
        &mut sm,
    );
    step(
        "outer-col",
        "K",
        Some((LogicalKey::K, Phase::Press)),
        &mut sm,
    );
    step(
        "outer-col-rel",
        "K",
        Some((LogicalKey::K, Phase::Release)),
        &mut sm,
    );
    step(
        "outer-row",
        "K",
        Some((LogicalKey::K, Phase::Press)),
        &mut sm,
    );
    step(
        "outer-row-rel",
        "K",
        Some((LogicalKey::K, Phase::Release)),
        &mut sm,
    );
    // fast three-key burst without releases: repeats must stay consumed
    step(
        "nested-press",
        "K",
        Some((LogicalKey::K, Phase::Press)),
        &mut sm,
    );
    step(
        "nested-repeat",
        "K",
        Some((LogicalKey::K, Phase::Press)),
        &mut sm,
    );
    step(
        "nested-release",
        "K",
        Some((LogicalKey::K, Phase::Release)),
        &mut sm,
    );
    // undo and cancel paths
    step(
        "rearm",
        "space",
        Some((LogicalKey::Space, Phase::Press)),
        &mut sm,
    );
    step(
        "outer-col",
        "K",
        Some((LogicalKey::K, Phase::Press)),
        &mut sm,
    );
    step(
        "outer-col-rel",
        "K",
        Some((LogicalKey::K, Phase::Release)),
        &mut sm,
    );
    step(
        "outer-row",
        "K",
        Some((LogicalKey::K, Phase::Press)),
        &mut sm,
    );
    step(
        "outer-row-rel",
        "K",
        Some((LogicalKey::K, Phase::Release)),
        &mut sm,
    );
    step(
        "undo-level",
        "backspace",
        Some((LogicalKey::Backspace, Phase::Press)),
        &mut sm,
    );
    step(
        "cancel",
        "esc",
        Some((LogicalKey::Esc, Phase::Press)),
        &mut sm,
    );
    // missing release: press caps, never release, Esc recovers
    step(
        "missing-rel",
        "caps",
        Some((LogicalKey::CapsLock, Phase::Press)),
        &mut sm,
    );
    sm.poll(10_000);
    step(
        "esc-recover",
        "esc",
        Some((LogicalKey::Esc, Phase::Press)),
        &mut sm,
    );
}

fn main() {
    run("100%", 1920, 1080);
    run("150%", 2880, 1620);
    run("200%", 3840, 2160);
}
