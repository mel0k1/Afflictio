use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tiocomp::kms::{self, DumbBuffer, KmsDevice};
use tiocomp::{compose, status_json, Window};

const STATUS_PATH: &str = "/run/afflictio/tocomp.json";
const FRAME_INTERVAL_MS: u64 = 16;
const STATUS_EVERY_FRAMES: u32 = 10;

fn main() {
    if let Err(e) = run() {
        eprintln!("tiocompd: {e}");
        std::process::exit(1);
    }
}

fn run() -> std::io::Result<()> {
    let Some(path) = KmsDevice::scan() else {
        eprintln!("tiocompd: no KMS device with a connected display, exiting");
        return Ok(());
    };
    let device = KmsDevice::open(&path)?;
    let Some(connector) = device.connected_connector() else {
        eprintln!("tiocompd: no connected connector on {path}");
        return Ok(());
    };
    let mode = kms::preferred_mode(&connector.modes);
    let (width, height) = mode.resolution();
    eprintln!(
        "tiocompd: {path} connector {} mode {} ({}x{})",
        connector.name,
        mode.mode_name(),
        width,
        height
    );

    let ((_, crtcs, _, _), _) = device.resources()?;
    let Some(crtc_id) = find_crtc(&device, &crtcs) else {
        eprintln!(
            "tiocompd: no crtc available for connector {}",
            connector.name
        );
        return Ok(());
    };

    let mut buffers: Vec<DumbBuffer> = Vec::new();
    for _ in 0..2 {
        let dumb = device.create_dumb(width, height)?;
        let mut mapped = device.map_dumb(dumb.handle, dumb.size)?;
        mapped.pitch = dumb.pitch;
        mapped.fb_id = device.add_fb2(width, height, dumb.handle, dumb.pitch)?;
        buffers.push(mapped);
    }
    let pitch_px = buffers[0].pitch / 4;

    let original = device.current_crtc(crtc_id)?;
    let windows = if width >= 640 && height >= 480 {
        vec![
            Window {
                x: width / 8,
                y: height / 4,
                w: width / 4,
                h: height / 3,
                color: tiocomp::rgb(68, 71, 90),
                label: String::from("SESSION VT1"),
            },
            Window {
                x: (width / 2) + width / 16,
                y: height / 4,
                w: width / 4,
                h: height / 3,
                color: tiocomp::rgb(80, 60, 110),
                label: String::from("SESSION VT2"),
            },
        ]
    } else {
        Vec::new()
    };

    compose_into(&mut buffers, 1, 0, width, height, pitch_px, &windows);
    if let Err(e) = device.set_crtc(crtc_id, buffers[1].fb_id, connector.id, mode) {
        eprintln!("tiocompd: set_crtc failed: {e}, leaving fbcon in place");
        return Ok(());
    }

    let running = Arc::new(AtomicBool::new(true));
    {
        let running = running.clone();
        std::thread::spawn(move || {
            let mut signals = Signals::new([SIGTERM, SIGINT]).expect("signal hook");
            for signal in signals.forever() {
                if signal == SIGTERM || signal == SIGINT {
                    running.store(false, Ordering::Relaxed);
                    break;
                }
            }
        });
    }

    let mut method = String::from("pageflip");
    let mut flips = 0u32;
    let mut frames = 1u32;
    write_status(&connector.name, &mode.mode_name(), &method, flips, frames);
    let mut back = 0usize;

    while running.load(Ordering::Relaxed) {
        compose_into(
            &mut buffers,
            back,
            frames,
            width,
            height,
            pitch_px,
            &windows,
        );
        let target_fb = buffers[back].fb_id;
        let ok = match device.page_flip(crtc_id, target_fb) {
            Ok(()) => true,
            Err(_) => match device.set_crtc(crtc_id, target_fb, connector.id, mode) {
                Ok(()) => {
                    if method != "setcrtc" {
                        method = String::from("setcrtc");
                    }
                    true
                }
                Err(_) => false,
            },
        };
        if ok {
            flips += 1;
        }
        back ^= 1;
        frames += 1;
        if frames.is_multiple_of(STATUS_EVERY_FRAMES) {
            write_status(&connector.name, &mode.mode_name(), &method, flips, frames);
        }
        std::thread::sleep(Duration::from_millis(FRAME_INTERVAL_MS));
    }

    let _ = device.set_crtc(
        crtc_id,
        original.fb_id,
        connector.id,
        if original.mode_valid != 0 {
            original.mode
        } else {
            mode
        },
    );
    for buffer in &buffers {
        device.rm_fb(buffer.fb_id);
        device.destroy_dumb(buffer.handle);
    }
    eprintln!("tiocompd: stopped after {frames} frames, {flips} flips via {method}");
    Ok(())
}

fn compose_into(
    buffers: &mut [DumbBuffer],
    index: usize,
    frame: u32,
    width: u32,
    height: u32,
    pitch_px: u32,
    windows: &[Window],
) {
    let pixels = buffers[index].pixels();
    compose(
        pixels,
        pitch_px,
        width,
        height,
        frame,
        windows,
        "AFFLICTIO KMS COMPOSITOR",
    );
}

fn find_crtc(device: &KmsDevice, crtcs: &[u32]) -> Option<u32> {
    for id in crtcs {
        if let Ok(current) = device.current_crtc(*id) {
            if current.fb_id != 0 {
                return Some(*id);
            }
        }
    }
    crtcs.first().copied()
}

fn write_status(connector: &str, mode: &str, method: &str, flips: u32, frames: u32) {
    if let Some(dir) = std::path::Path::new(STATUS_PATH).parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(
        STATUS_PATH,
        status_json(connector, mode, method, flips, frames),
    );
}
