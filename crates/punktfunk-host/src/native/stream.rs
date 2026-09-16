//! Native `punktfunk/1` capture→encode→send data plane.
//!
//! Owns the synthetic and software protocol-test sources, speed-test probes, the paced submit,
//! and [`SessionContext`], the per-session inputs `serve_session` hands to [`virtual_stream`].
//! The virtual-display loop itself is [`state::StreamState`]: bring-up in `new`, the tick loop
//! in `run`, one file per concern under `stream/`.
//!
//! Pin `PUNKTFUNK_PHASE_LOCK=0`, `PUNKTFUNK_IDD_ADAPTIVE=0`, `PUNKTFUNK_PACE_FACTOR=0`,
//! `PUNKTFUNK_STREAMED_AU=0` for the rebuild-free A/B levers. Evidence:
//! `design/phase-locked-capture.md`, `design/midstream-resolution-resize.md`.

use super::*;
use crate::send_pacing::{frame_driven_enabled, CaptureCredit};

mod cursor;
mod encode;
mod phase_lock;
mod pipeline;
mod rebuild;
mod recovery;
#[cfg(target_os = "windows")]
mod resize;
mod send;
mod session_watch;
mod state;
use self::phase_lock::{phase_lock_enabled, PhaseController};
// `native.rs` builds it and `control.rs` holds it: the 0xCF ACK hold crosses the module.
pub(crate) use self::phase_lock::PhaseCtl;
pub(super) use self::pipeline::{prepare_display, PrepHandle, PreparedDisplay};
use self::send::{send_loop, ChunkMsg, FrameMsg, SendMsg, SendStats};
// `native.rs` asks before offering a mid-stream reconfig.
pub(crate) use self::send::reconfig_allowed;
use self::session_watch::{session_watch_enabled, session_watcher_loop, SessionSwitch};
use self::state::StreamState;

#[allow(clippy::too_many_arguments)]
pub(super) fn synthetic_stream(
    session: &mut Session,
    frames: u32,
    stop: &AtomicBool,
    probe_rx: &std::sync::mpsc::Receiver<ProbeRequest>,
    probe_result_tx: &tokio::sync::mpsc::UnboundedSender<ProbeResult>,
    fec_target: &AtomicU8,
    timing_conn: Option<&super::link::SessionLink>,
    probe_seq: bool,
) -> Result<()> {
    let interval = std::time::Duration::from_millis(1000 / 60);
    for idx in 0..frames {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        apply_fec_target(session, fec_target);
        service_probes(session, stop, probe_rx, probe_result_tx, probe_seq);
        let data = test_frame(idx, 64 * 1024);
        let pts_ns = now_ns();
        session
            .submit_frame(&data, pts_ns, (FLAG_PIC | FLAG_SOF) as u32)
            .map_err(|e| anyhow!("submit_frame: {e:?}"))?;
        // 0xCF host_us is near-zero here (no capture/encode); the datagram still proves the plane.
        if let Some(tc) = timing_conn {
            let t = punktfunk_core::quic::HostTiming {
                pts_ns,
                host_us: (now_ns().saturating_sub(pts_ns) / 1000).min(u32::MAX as u64) as u32,
                stages: None,
                applied_phase_ns: None,
            };
            let _ = tc.send_datagram(punktfunk_core::quic::encode_host_timing_datagram(&t));
        }
        std::thread::sleep(interval);
    }
    tracing::info!(frames, "synthetic stream complete");
    Ok(())
}

/// A moving picture through the software H.264 encoder until the session stops. Linux only,
/// because the encoder is; elsewhere a client gets a refusal it can read, not a hang.
///
/// Named, not resolved from the ladder: `auto` never picks software, and `PUNKTFUNK_ENCODER`
/// is latched long before a session arrives.
#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
pub(super) fn software_stream(
    session: &mut Session,
    codec: crate::encode::Codec,
    mode: punktfunk_core::config::Mode,
    bitrate_kbps: u32,
    stop: &AtomicBool,
    probe_rx: &std::sync::mpsc::Receiver<ProbeRequest>,
    probe_result_tx: &tokio::sync::mpsc::UnboundedSender<ProbeResult>,
    fec_target: &AtomicU8,
    probe_seq: bool,
) -> Result<()> {
    use crate::capture::Capturer;
    anyhow::ensure!(
        codec == crate::encode::Codec::H264,
        "the software source encodes H.264 only, and {codec:?} was negotiated"
    );
    let (w, h, fps) = (mode.width, mode.height, mode.refresh_hz.max(1));
    let mut capturer = crate::capture::SyntheticCapturer::new(w, h, fps);
    let mut frame = capturer.next_frame().context("first synthetic frame")?;
    let mut encoder =
        pf_encode::open_software_h264(frame.format, w, h, fps, u64::from(bitrate_kbps) * 1000)
            .context("open the software encoder")?;
    let interval = std::time::Duration::from_nanos(1_000_000_000 / u64::from(fps));
    let mut frames = 0u64;
    while !stop.load(Ordering::SeqCst) {
        apply_fec_target(session, fec_target);
        service_probes(session, stop, probe_rx, probe_result_tx, probe_seq);
        encoder.submit(&frame).context("encode submit")?;
        while let Some(au) = encoder.poll().context("encode poll")? {
            let mut flags = u32::from(FLAG_PIC);
            if au.keyframe {
                flags |= u32::from(FLAG_SOF);
            }
            if session.submit_frame(&au.data, au.pts_ns, flags).is_err() {
                tracing::info!(frames, "software stream ended: the transport refused");
                return Ok(());
            }
            frames += 1;
        }
        std::thread::sleep(interval);
        frame = capturer.next_frame().context("synthetic frame")?;
    }
    tracing::info!(frames, "software stream complete");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
#[allow(clippy::too_many_arguments)]
pub(super) fn software_stream(
    _session: &mut Session,
    _codec: crate::encode::Codec,
    _mode: punktfunk_core::config::Mode,
    _bitrate_kbps: u32,
    _stop: &AtomicBool,
    _probe_rx: &std::sync::mpsc::Receiver<ProbeRequest>,
    _probe_result_tx: &tokio::sync::mpsc::UnboundedSender<ProbeResult>,
    _fec_target: &AtomicU8,
    _probe_seq: bool,
) -> Result<()> {
    anyhow::bail!("the software source needs the software encoder, which is Linux-only")
}

/// Probe ceiling: 10 Gbps / 5 s. Above the session cap ([`MAX_BITRATE_KBPS`], 2 Gbps) so a
/// probe can show headroom past the rate a session will actually use.
const MAX_PROBE_KBPS: u32 = 10_000_000;
const MAX_PROBE_MS: u32 = 5_000;

/// Burst zero-filled [`FLAG_PROBE`] AUs at `req.target_kbps` for `req.duration_ms` (clamped to
/// `MAX_PROBE_*`). Paces by a bytes-allowed-so-far budget so scheduling jitter does not overshoot.
/// Video is paused for the duration — the caller's loop is blocked here.
fn run_probe_burst(
    session: &mut Session,
    req: ProbeRequest,
    stop: &AtomicBool,
    probe_seq: bool,
) -> ProbeResult {
    let target_kbps = req.target_kbps.min(MAX_PROBE_KBPS);
    let duration_ms = req.duration_ms.min(MAX_PROBE_MS);
    // Probe filler uses its own frame-index space. Without VIDEO_CAP_PROBE_SEQ the client has
    // one reassembly window and would drop probe frames as stale — decline rather than consume
    // video indexes the gap detector would read as a multi-thousand-frame loss after the burst.
    if !probe_seq {
        tracing::info!(
            "declining speed-test probe: client predates VIDEO_CAP_PROBE_SEQ (its reassembler \
             cannot window probe-space frames)"
        );
        return ProbeResult {
            bytes_sent: 0,
            packets_sent: 0,
            duration_ms: 0,
            wire_packets_sent: 0,
            send_dropped: 0,
        };
    }
    if target_kbps == 0 || duration_ms == 0 {
        return ProbeResult {
            bytes_sent: 0,
            packets_sent: 0,
            duration_ms: 0,
            wire_packets_sent: 0,
            send_dropped: 0,
        };
    }
    let bytes_per_sec = target_kbps as u64 * 125;
    // ≤16 KiB ≈ a dozen MTU shards; a 256 KiB AU overflowed a ~400 KiB send buffer on one submit.
    let chunk = (bytes_per_sec / 240).clamp(1200, 16 * 1024) as usize;
    let filler = vec![0u8; chunk];
    // Video is paused here, so the sealed/dropped deltas isolate host-side drops from link loss.
    let wire0 = session.stats().packets_sent;
    let drop0 = session.stats().packets_send_dropped;
    let start = std::time::Instant::now();
    let deadline = start + std::time::Duration::from_millis(duration_ms as u64);
    let mut bytes_sent = 0u64;
    let mut packets_sent = 0u32;
    while std::time::Instant::now() < deadline && !stop.load(Ordering::SeqCst) {
        let allowed = (start.elapsed().as_secs_f64() * bytes_per_sec as f64) as u64;
        if bytes_sent < allowed {
            // WouldBlock/ENOBUFS is part of what the probe measures (`send_dropped`) — keep going.
            let _ = session.submit_probe_frame(&filler, now_ns());
            bytes_sent += chunk as u64;
            packets_sent += 1;
        } else {
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
    }
    let actual_ms = start.elapsed().as_millis() as u32;
    let wire_offered = (session.stats().packets_sent - wire0) as u32;
    let send_dropped = (session.stats().packets_send_dropped - drop0) as u32;
    let wire_packets_sent = wire_offered.saturating_sub(send_dropped);
    tracing::info!(
        target_kbps,
        duration_ms = actual_ms,
        bytes_sent,
        au_count = packets_sent,
        wire_offered,
        wire_packets_sent,
        send_dropped,
        "speed-test probe burst complete"
    );
    ProbeResult {
        bytes_sent,
        packets_sent,
        duration_ms: actual_ms,
        wire_packets_sent,
        send_dropped,
    }
}

/// Drain pending speed-test requests between frames. `probe_seq` is [`VIDEO_CAP_PROBE_SEQ`].
fn service_probes(
    session: &mut Session,
    stop: &AtomicBool,
    probe_rx: &std::sync::mpsc::Receiver<ProbeRequest>,
    probe_result_tx: &tokio::sync::mpsc::UnboundedSender<ProbeResult>,
    probe_seq: bool,
) {
    while let Ok(req) = probe_rx.try_recv() {
        let result = run_probe_burst(session, req, stop, probe_seq);
        let _ = probe_result_tx.send(result);
    }
}

/// Seal one AU and send it under [`send_pacing`](crate::send_pacing): first `burst_cap` bytes
/// leave immediately; overflow spreads at `pace_rate_bps` in adaptive chunks (16…64, the GSO
/// cap). `burst_cap` `None` = 10 ms at the pace rate, clamped to [16 KiB, 256 KiB]
/// ([`crate::send_pacing::auto_burst_bytes`]); `Some` = `PUNKTFUNK_PACE_BURST_KB`. An unpaced
/// line-rate burst overruns the kernel tx buffer → EAGAIN → freeze until the next keyframe.
///
/// `pace_rate_bps` is ~3× the live encoder bitrate — the overflow's wire time at that rate is
/// the budget ([`crate::send_pacing::native_budget`], [`MAX_PACE_SPREAD`]-bounded). `0` =
/// deadline-only spread (`PUNKTFUNK_PACE_FACTOR=0`, or bitrate not yet known).
#[allow(clippy::too_many_arguments)]
fn paced_submit(
    session: &mut Session,
    data: &[u8],
    pts_ns: u64,
    flags: u32,
    frame_index: u32,
    deadline: std::time::Instant,
    burst_cap: Option<usize>,
    pace_rate_bps: u64,
    max_spread: std::time::Duration,
) -> Result<PaceStat> {
    let wires = session
        .seal_frame_at(data, pts_ns, flags, frame_index)
        .map_err(|e| anyhow!("seal_frame: {e:?}"))?;
    pace_sealed(
        session,
        wires,
        deadline,
        burst_cap,
        pace_rate_bps,
        max_spread,
    )
}

/// Pace already-sealed wires. Shared with the streamed-AU path ([`handle_chunk`]).
fn pace_sealed(
    session: &mut Session,
    wires: Vec<Vec<u8>>,
    deadline: std::time::Instant,
    burst_cap: Option<usize>,
    pace_rate_bps: u64,
    max_spread: std::time::Duration,
) -> Result<PaceStat> {
    let mut refs: Vec<&[u8]> = wires.iter().map(|w| w.as_slice()).collect();
    crate::send_pacing::inject_video_drop(&mut refs);
    let wire_bytes: usize = refs.iter().map(|p| p.len()).sum();
    let burst_bytes = burst_cap
        .unwrap_or_else(|| crate::send_pacing::auto_burst_bytes(pace_rate_bps, wire_bytes));
    let cfg = crate::send_pacing::PaceCfg {
        burst_bytes: Some(burst_bytes),
        chunk: crate::send_pacing::ChunkPolicy::Adaptive { base: 16, max: 64 },
        sleep_floor: std::time::Duration::from_micros(500),
    };
    let overflow_bytes = wire_bytes.saturating_sub(burst_bytes) as u64;
    let budget =
        crate::send_pacing::native_budget(deadline, pace_rate_bps, overflow_bytes, max_spread);
    // Sleeps between chunks stay excluded: sock_ns is pure send_gso/sendmmsg time.
    let mut sock_ns = 0u64;
    let result = crate::send_pacing::pace_frame(&refs, budget, &cfg, |chunk| {
        let t0 = std::time::Instant::now();
        let r = session.send_sealed(chunk).map(|_| ());
        sock_ns += t0.elapsed().as_nanos() as u64;
        r
    });
    drop(refs);
    session.reclaim_wires(wires);
    session.note_sock_ns(sock_ns);
    result.map_err(|e| anyhow!("send_sealed: {e:?}"))
}

/// Owned per-session inputs for [`virtual_stream`]. Receivers move in; the whole context moves
/// onto the stream thread.
pub(super) struct SessionContext {
    pub(super) session: Session,
    pub(super) mode: punktfunk_core::Mode,
    pub(super) seconds: u32,
    pub(super) stop: Arc<AtomicBool>,
    /// Set on `QUIT_CODE`. Display lease skips keep-alive linger for a user stop.
    pub(super) quit: Arc<AtomicBool>,
    /// [`crate::events::SessionEndReason`] latch for the session summary; first write wins.
    pub(super) end_reason: Arc<std::sync::atomic::AtomicU8>,
    /// Session totals for the summary; the encode loop notes every bitrate it runs at.
    pub(super) counters: Arc<crate::session_status::SessionCounters>,
    pub(super) reconfig: std::sync::mpsc::Receiver<punktfunk_core::Mode>,
    pub(super) keyframe: std::sync::mpsc::Receiver<()>,
    /// Lost-frame range `(first, last)`. Prefer `invalidate_ref_frames` over a full IDR.
    pub(super) rfi: std::sync::mpsc::Receiver<(u32, u32)>,
    pub(super) bitrate_rx: std::sync::mpsc::Receiver<u32>,
    /// Validated + ack-gated by the wire-MTU watcher. Applied between AUs only.
    pub(super) shard_rx: std::sync::mpsc::Receiver<usize>,
    pub(super) compositor: crate::vdisplay::Compositor,
    /// Per-instance, not via `PUNKTFUNK_GAMESCOPE_NODE` — two sessions must not overwrite each other.
    pub(super) gamescope_route: Option<crate::vdisplay::GamescopeRoute>,
    /// Total wire budget (kbps): video + FEC + framing + audio reservation. PyroWave is identity.
    pub(super) bitrate_kbps: u32,
    pub(super) audio_reserved_kbps: u32,
    pub(super) shard_payload: u16,
    /// ASIC-applied rate, not the request. Shared with pacer, console, mgmt, and climb acks.
    pub(super) live_bitrate: Arc<AtomicU32>,
    /// 0 = none discovered. A request already at the ceiling costs nothing to apply.
    pub(super) encoder_ceiling_kbps: Arc<AtomicU32>,
    /// While set, refuse bitrate climbs — the network is not the bottleneck.
    pub(super) cadence_degraded: Arc<AtomicBool>,
    pub(super) cadence_behind_score: Arc<AtomicU32>,
    /// [`u32::MAX`] = client too old to send a [`DeliveryReport`]. Distinguishes clean-link from
    /// nothing-arriving: both look like `loss_ppm = 0`.
    pub(super) client_packets_received: Arc<AtomicU32>,
    /// `Hello::bitrate_kbps == 0`. PyroWave re-resolves on a mid-stream mode switch; an explicit rate stays.
    pub(super) bitrate_auto: bool,
    /// 8 or 10. Does not imply HDR — `hdr` is separate (10-bit SDR path).
    pub(super) bit_depth: u8,
    pub(super) hdr: bool,
    pub(super) chroma: crate::encode::ChromaFormat,
    pub(super) codec: crate::encode::Codec,
    pub(super) probe_rx: std::sync::mpsc::Receiver<ProbeRequest>,
    pub(super) probe_result_tx: tokio::sync::mpsc::UnboundedSender<ProbeResult>,
    /// Corrective `Reconfigured` when a rebuild stayed at the old mode or honored a different refresh.
    pub(super) reconfig_result_tx: tokio::sync::mpsc::UnboundedSender<Reconfigured>,
    pub(super) retarget_tx: tokio::sync::mpsc::UnboundedSender<u32>,
    pub(super) gap_tx: tokio::sync::mpsc::UnboundedSender<u32>,
    pub(super) fec_target: Arc<AtomicU8>,
    pub(super) conn: super::link::SessionLink,
    pub(super) timing_conn: Option<super::link::SessionLink>,
    pub(super) phase: Arc<PhaseCtl>,
    pub(super) cursor_forward: bool,
    /// `true` = client draws; `false` = host composites. Always `true` (inert) for non-cap sessions.
    pub(super) cursor_client_draws: Arc<AtomicBool>,
    /// Depth-1 latest-wins; see [`super::cursor_fwd::CursorForwarder::tick`].
    pub(super) cursor_shape_tx:
        tokio::sync::watch::Sender<Option<punktfunk_core::quic::CursorShape>>,
    /// Without this, a mid-session probe consumes video indexes the gap detector cannot see.
    pub(super) probe_seq: bool,
    pub(super) streamed_au: bool,
    /// `false` = single-slice. TV-SoC decoders (Amlogic) wedge on multi-slice.
    pub(super) multi_slice: bool,
    pub(super) stats: Arc<StatsRecorder>,
    pub(super) client_label: String,
    pub(super) client_name: Option<String>,
    pub(super) launch: Option<String>,
    pub(super) launch_target: Option<crate::library::LaunchTarget>,
    /// Where this session's launch outcome goes; the control task writes it to
    /// the client ([`punktfunk_core::quic::LaunchOutcome`]).
    pub(super) launch_outcome: crate::gamelease::OutcomeTx,
    /// Threaded into the EDID CTA HDR block before `create` so host apps tone-map to the client's panel.
    pub(super) client_hdr: Option<pf_frame::HdrMeta>,
    /// Admitted by `mode_conflict: join`: share the live display instead of creating one.
    pub(super) join_live: bool,
    /// Per-session handles the management routes act on; published to the registry.
    pub(super) controls: crate::session_status::SessionControls,
    /// A joiner's view and fit ([`SessionPlan::reframe_to`](crate::session_plan::SessionPlan::reframe_to)).
    pub(super) reframe_to: Option<(punktfunk_core::video_fit::VideoFit, (u32, u32))>,
    /// The encoder's framing, published for the input thread.
    pub(super) frame_map: super::input::FrameMap,
    pub(super) bringup: Arc<crate::bringup::Trace>,
    pub(super) resize_ms: Arc<AtomicU32>,
    /// A clone of the data socket for the sender's kernel-queue probe; `None` on the web plane.
    pub(super) wire_sock: Option<std::net::UdpSocket>,
    #[cfg(target_os = "linux")]
    pub(super) input_tx: std::sync::mpsc::SyncSender<super::input::ClientInput>,
    /// Isolated gamescope spawn identity. `None` = shared planes. See `design/gamescope-multiuser.md`.
    #[cfg(target_os = "linux")]
    pub(super) isolation: Option<crate::vdisplay::SessionIsolation>,
    #[cfg(target_os = "linux")]
    pub(super) input_route: super::input::InputRoute,
    #[cfg(target_os = "linux")]
    pub(super) inj_shared_tx: std::sync::mpsc::Sender<punktfunk_core::input::InputEvent>,
    #[cfg(target_os = "linux")]
    pub(super) inj_session_tx: Option<std::sync::mpsc::Sender<punktfunk_core::input::InputEvent>>,
}

/// The virtual-display session: bring up, then tick until the client leaves.
pub(super) fn virtual_stream(ctx: SessionContext, prepared: Option<PreparedDisplay>) -> Result<()> {
    boost_thread_priority(true);
    StreamState::new(ctx, prepared)?.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconfig_allowed_gates_gamescope_and_per_client_mode() {
        use crate::vdisplay::Compositor::{Gamescope, Hyprland, Kwin, Mutter, Wlroots};
        assert!(!reconfig_allowed(Some(Gamescope), false, false));
        assert!(!reconfig_allowed(Some(Gamescope), true, false));
        assert!(!reconfig_allowed(Some(Kwin), true, false));
        assert!(!reconfig_allowed(Some(Mutter), true, false));
        assert!(!reconfig_allowed(None, true, false));
        for c in [Kwin, Mutter, Wlroots, Hyprland] {
            assert!(
                reconfig_allowed(Some(c), false, false),
                "{c:?} should allow live reconfigure"
            );
        }
        assert!(reconfig_allowed(None, false, false));
    }

    #[test]
    fn reconfig_allowed_rejects_a_monitor_mirror_on_every_backend() {
        use crate::vdisplay::Compositor::{Hyprland, Kwin, Mutter, Wlroots};
        for c in [Kwin, Mutter, Wlroots, Hyprland] {
            assert!(
                reconfig_allowed(Some(c), false, false),
                "{c:?} without a pin should still allow live reconfigure"
            );
            assert!(
                !reconfig_allowed(Some(c), false, true),
                "{c:?} mirroring a physical head must reject a resize"
            );
        }
    }
}
