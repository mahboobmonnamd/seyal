use std::{
    net::Shutdown,
    os::unix::net::UnixStream,
    thread,
    time::{Duration, Instant},
};

use seyal_protocol::pass8::{BLOCK_STATE_MESSAGE_TYPE, CAP_BLOCK_METADATA};
use seyal_render::PreparedSurface;
use seyal_runtime::{
    AttachmentId,
    display::{DecodedDisplayChunk, decode_chunk, empty_cache},
    local_ipc::framing::{
        Attach, Attached, CAP_COMMAND_BLOCKS, ClientHello, Detach, Detached, ExecutionList,
        MessageType, Role, ServerHello, encode_frame,
    },
};

use super::config::{
    Geometry, LossMode, MEASURED_CYCLES, RSS_TAIL_CYCLES, SETTLE_TIMEOUT, WARMUP_CYCLES,
};
use super::metrics::{ProcessMetrics, elapsed_ns, median_self_metrics, self_metrics, stats};
use super::protocol::{panic_server_error, prepare_surface, read_frame, read_until, send_frame};
use super::worker::RuntimeWorker;
use crate::PERFORMANCE_CLAIM;

pub(crate) fn run_cohort(mode: LossMode, geometry: Geometry, cohort: usize) {
    let mut worker = RuntimeWorker::start(geometry);
    let client_baseline = median_self_metrics();
    let runtime_baseline = worker.median_metrics();
    assert_quiescent(&mut worker, runtime_baseline, client_baseline);

    let mut previous_attachment = None;
    for _ in 0..WARMUP_CYCLES {
        let attachment = open_attachment(&worker, geometry);
        assert_fresh_attachment(&mut previous_attachment, attachment.attachment_id);
        cleanup_attachment(mode, &mut worker, attachment);
        let _ = worker.read_cleanup_sample();
        assert_quiescent(&mut worker, runtime_baseline, client_baseline);
    }

    let runtime_rss_baseline = worker.median_metrics().rss_kib;
    let client_rss_baseline = median_self_metrics().rss_kib;
    let mut reconnect_samples = Vec::with_capacity(MEASURED_CYCLES);
    let mut renderer_ready_samples = Vec::with_capacity(MEASURED_CYCLES);
    let mut cleanup_samples = Vec::with_capacity(MEASURED_CYCLES);
    let mut runtime_rss_cycles = Vec::with_capacity(MEASURED_CYCLES);
    let mut client_rss_cycles = Vec::with_capacity(MEASURED_CYCLES);

    for _ in 0..MEASURED_CYCLES {
        let attachment = open_attachment(&worker, geometry);
        reconnect_samples.push(attachment.reconnect_ns);
        renderer_ready_samples.push(attachment.renderer_ready_ns);
        assert_fresh_attachment(&mut previous_attachment, attachment.attachment_id);

        // Attached.granted_role is the authoritative controller grant. Runtime
        // metrics independently verify the logical attachment and the one live
        // lifecycle socket without creating a diagnostic local-IPC connection
        // that could contaminate the exact disconnect cleanup timing sample.
        let attached_metrics = worker.metrics();
        assert_eq!(attached_metrics.attachment_count, 1);
        assert!(attached_metrics.has_controller);
        assert_eq!(attached_metrics.local_connection_count, 1);
        assert_eq!(attached_metrics.pending_resync_count, 0);
        assert_eq!(attached_metrics.pending_resync_set_count, 0);
        assert!(!attached_metrics.listener_backoff_active);
        assert_eq!(attached_metrics.threads, runtime_baseline.threads);
        assert_eq!(attached_metrics.fds, runtime_baseline.fds + 1);

        cleanup_attachment(mode, &mut worker, attachment);
        cleanup_samples.push(worker.read_cleanup_sample());
        assert_quiescent(&mut worker, runtime_baseline, client_baseline);
        runtime_rss_cycles.push(worker.metrics().rss_kib);
        client_rss_cycles.push(self_metrics().rss_kib);
    }

    let runtime_final = worker.median_metrics();
    let client_final = median_self_metrics();
    let idle_cpu = worker.measure_idle_cpu();
    let reconnect = stats(&mut reconnect_samples);
    let renderer_ready = stats(&mut renderer_ready_samples);
    let cleanup = stats(&mut cleanup_samples);
    let runtime_rss_delta_kib = runtime_final.rss_kib as i64 - runtime_rss_baseline as i64;
    let client_rss_delta_kib = client_final.rss_kib as i64 - client_rss_baseline as i64;
    let runtime_cycle_growth = signed_growth(&runtime_rss_cycles);
    let client_cycle_growth = signed_growth(&client_rss_cycles);
    let runtime_cycle_slope = linear_slope(&runtime_rss_cycles);
    let client_cycle_slope = linear_slope(&client_rss_cycles);
    let runtime_tail_growth = tail_growth(&runtime_rss_cycles, RSS_TAIL_CYCLES);
    let client_tail_growth = tail_growth(&client_rss_cycles, RSS_TAIL_CYCLES);

    println!(
        "pass9_calibration_cohort mode={} geometry={} cohort={} runtime_pid={} runtime_id={} execution_id={} reconnect_boundary=local_connect_hello_resolve_attach_to_complete_authoritative_client_commit reconnect_p50_us={:.3} reconnect_p95_us={:.3} reconnect_p99_us={:.3} reconnect_max_us={:.3} renderer_ready_boundary=committed_client_state_to_PreparedSurface_ready renderer_p50_us={:.3} renderer_p95_us={:.3} renderer_p99_us={:.3} renderer_max_us={:.3} cleanup_boundary=Runtime_disconnect_or_Detach_dispatch_to_attachment_controller_cleanup cleanup_classification=MEASURED_EXACT_RUNTIME_DISPATCH cleanup_p50_us={:.3} cleanup_p95_us={:.3} cleanup_p99_us={:.3} cleanup_max_us={:.3} runtime_rss_baseline_kib={} runtime_rss_final_kib={} runtime_rss_delta_kib={} runtime_cycle_rss_growth_kib={} runtime_rss_tail_cycles={} runtime_rss_tail_growth_kib={} client_rss_baseline_kib={} client_rss_final_kib={} client_rss_delta_kib={} client_cycle_rss_growth_kib={} client_rss_tail_cycles={} client_rss_tail_growth_kib={} runtime_fds_baseline={} runtime_fds_final={} runtime_threads_baseline={} runtime_threads_final={} client_fds_baseline={} client_fds_final={} client_threads_baseline={} client_threads_final={} idle_runtime_cpu_percent={:.3} controller_authority_source=Attached_granted_role_plus_Runtime_benchmark_diagnostics attachment_controller_fd_thread_return_each_cycle=true client_socket_closed_each_cycle=true pending_resync_work=Runtime_diagnostics_verified_zero retry_work=NOT_IMPLEMENTED_IN_PRE_PASS9_BASELINE runtime_lifecycle_quiescence=two_stable_direct_Runtime_diagnostic_samples sample_count={} {}",
        mode.label(),
        geometry.label(),
        cohort,
        worker.pid,
        worker.runtime_id,
        u128::from_le_bytes(worker.execution_id.to_bytes()),
        reconnect.p50_us,
        reconnect.p95_us,
        reconnect.p99_us,
        reconnect.max_us,
        renderer_ready.p50_us,
        renderer_ready.p95_us,
        renderer_ready.p99_us,
        renderer_ready.max_us,
        cleanup.p50_us,
        cleanup.p95_us,
        cleanup.p99_us,
        cleanup.max_us,
        runtime_rss_baseline,
        runtime_final.rss_kib,
        runtime_rss_delta_kib,
        runtime_cycle_growth,
        RSS_TAIL_CYCLES,
        runtime_tail_growth,
        client_rss_baseline,
        client_final.rss_kib,
        client_rss_delta_kib,
        client_cycle_growth,
        RSS_TAIL_CYCLES,
        client_tail_growth,
        runtime_baseline.fds,
        runtime_final.fds,
        runtime_baseline.threads,
        runtime_final.threads,
        client_baseline.fds,
        client_final.fds,
        client_baseline.threads,
        client_final.threads,
        idle_cpu,
        MEASURED_CYCLES,
        PERFORMANCE_CLAIM,
    );
    println!(
        "pass9_calibration_rss_samples mode={} geometry={} cohort={} sample_count={} runtime_rss_kib={} runtime_slope_kib_per_cycle={:.6} runtime_tail_cycles={} runtime_tail_growth_kib={} client_rss_kib={} client_slope_kib_per_cycle={:.6} client_tail_cycles={} client_tail_growth_kib={} {}",
        mode.label(),
        geometry.label(),
        cohort,
        MEASURED_CYCLES,
        comma_separated(&runtime_rss_cycles),
        runtime_cycle_slope,
        RSS_TAIL_CYCLES,
        runtime_tail_growth,
        comma_separated(&client_rss_cycles),
        client_cycle_slope,
        RSS_TAIL_CYCLES,
        client_tail_growth,
        PERFORMANCE_CLAIM,
    );
    println!(
        "PASS9_RESULT reconnect_p99_us={:.3} renderer_ready_p99_us={:.3} cleanup_p99_us={:.3} runtime_rss_delta_kib={} client_rss_delta_kib={}",
        reconnect.p99_us,
        renderer_ready.p99_us,
        cleanup.p99_us,
        runtime_rss_delta_kib,
        client_rss_delta_kib,
    );
    worker.finish();
}

fn signed_growth(samples: &[usize]) -> i64 {
    match (samples.first(), samples.last()) {
        (Some(first), Some(last)) => *last as i64 - *first as i64,
        _ => 0,
    }
}

fn linear_slope(samples: &[usize]) -> f64 {
    if samples.len() < 2 {
        return 0.0;
    }
    let count = samples.len() as f64;
    let mean_x = (count - 1.0) / 2.0;
    let mean_y = samples.iter().map(|value| *value as f64).sum::<f64>() / count;
    let (numerator, denominator) =
        samples
            .iter()
            .enumerate()
            .fold((0.0, 0.0), |(numerator, denominator), (index, value)| {
                let delta_x = index as f64 - mean_x;
                (
                    numerator + delta_x * (*value as f64 - mean_y),
                    denominator + delta_x * delta_x,
                )
            });
    numerator / denominator
}

fn tail_growth(samples: &[usize], tail_cycles: usize) -> i64 {
    assert!(tail_cycles > 0, "RSS tail window must be non-zero");
    let tail = &samples[samples.len().saturating_sub(tail_cycles)..];
    signed_growth(tail)
}

pub(crate) fn assert_rss_tail_measurement_integrity() {
    assert_eq!(tail_growth(&[100, 112, 112, 112], 3), 0);
    assert_eq!(tail_growth(&[100, 112, 120, 128], 3), 16);
    assert_eq!(tail_growth(&[100, 112], 25), 12);
}

fn comma_separated(samples: &[usize]) -> String {
    samples
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn assert_fresh_attachment(previous: &mut Option<AttachmentId>, current: AttachmentId) {
    if let Some(previous) = previous {
        assert_ne!(*previous, current, "AttachmentId reused across reconnect");
    }
    *previous = Some(current);
}

fn assert_quiescent(
    worker: &mut RuntimeWorker,
    runtime_baseline: ProcessMetrics,
    client_baseline: ProcessMetrics,
) {
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    loop {
        let first = worker.metrics();
        let client = self_metrics();
        if lifecycle_is_quiescent(first, runtime_baseline, client, client_baseline) {
            thread::sleep(Duration::from_millis(10));
            let second = worker.metrics();
            if lifecycle_is_quiescent(second, runtime_baseline, self_metrics(), client_baseline) {
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "Pass 9 lifecycle failed to quiesce"
        );
        thread::yield_now();
    }
}

fn lifecycle_is_quiescent(
    runtime: ProcessMetrics,
    runtime_baseline: ProcessMetrics,
    client: ProcessMetrics,
    client_baseline: ProcessMetrics,
) -> bool {
    runtime.attachment_count == 0
        && !runtime.has_controller
        && runtime.local_connection_count == 0
        && runtime.pending_resync_count == 0
        && runtime.pending_resync_set_count == 0
        && !runtime.listener_backoff_active
        && runtime.fds == runtime_baseline.fds
        && runtime.threads == runtime_baseline.threads
        && client.fds == client_baseline.fds
        && client.threads == client_baseline.threads
}

struct RawAttachment {
    stream: UnixStream,
    attachment_id: AttachmentId,
    reconnect_ns: u64,
    renderer_ready_ns: u64,
}

fn open_attachment(worker: &RuntimeWorker, geometry: Geometry) -> RawAttachment {
    let reconnect_started = Instant::now();
    let mut stream = UnixStream::connect(&worker.socket_path).expect("connect Runtime socket");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .expect("write timeout");

    send_frame(
        &mut stream,
        MessageType::ClientHello,
        &ClientHello {
            client_capabilities: CAP_COMMAND_BLOCKS | CAP_BLOCK_METADATA,
        }
        .encode(),
    );
    let hello_payload = read_until(&mut stream, MessageType::ServerHello as u16);
    let hello = ServerHello::decode(&hello_payload).expect("ServerHello decode");
    assert_eq!(hello.runtime_id, worker.runtime_id);

    send_frame(&mut stream, MessageType::ListExecutions, &[]);
    let list_payload = read_until(&mut stream, MessageType::ExecutionList as u16);
    let list = ExecutionList::decode(&list_payload).expect("ExecutionList decode");
    assert!(
        list.entries
            .iter()
            .any(|entry| entry.execution_id == worker.execution_id),
        "target execution missing during reconnect resolve"
    );

    send_frame(
        &mut stream,
        MessageType::Attach,
        &Attach {
            execution_id: worker.execution_id,
            requested_role: Role::Controller,
        }
        .encode(),
    );

    let mut attached = None;
    let mut chunks = Vec::<DecodedDisplayChunk>::new();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        assert!(
            Instant::now() < deadline,
            "authoritative snapshot timed out"
        );
        let (kind, payload) = read_frame(&mut stream);
        if kind == MessageType::Attached as u16 {
            attached = Some(Attached::decode(&payload).expect("Attached decode"));
        } else if kind == MessageType::DisplaySnapshot as u16 {
            // `read_frame` strips the wire header (matching the contract of
            // every other `*::decode` call in this loop), but `decode_chunk`
            // re-parses that header itself and expects it still attached —
            // re-encoding the already-stripped payload restores it, matching
            // the pattern used by local_ipc_protocol.rs and
            // pass7_local_ipc.rs.
            let chunk = decode_chunk(&encode_frame(MessageType::DisplaySnapshot, &payload))
                .expect("DisplaySnapshot decode");
            assert_eq!(chunk.rows, geometry.rows);
            assert_eq!(chunk.columns, geometry.columns);
            let complete = chunk.chunk_index + 1 == chunk.chunk_count;
            chunks.push(chunk);
            if complete && attached.is_some() {
                break;
            }
        } else if kind == BLOCK_STATE_MESSAGE_TYPE {
            continue;
        } else if kind == MessageType::Error as u16 {
            panic_server_error(&payload, "attach/snapshot");
        } else {
            panic!("unexpected frame type {kind} during authoritative reconnect");
        }
    }

    let attached = attached.expect("Attached frame");
    assert_eq!(attached.execution_id, worker.execution_id);
    assert_eq!(attached.granted_role, Role::Controller);
    let mut cache = empty_cache();
    cache
        .apply_chunks(&chunks)
        .expect("authoritative client cache commit");
    assert_eq!(cache.rows, geometry.rows);
    assert_eq!(cache.columns, geometry.columns);
    assert_eq!(cache.generation, attached.current_generation);
    let committed_at = Instant::now();
    let reconnect_ns = elapsed_ns(reconnect_started);

    let mut prepared = PreparedSurface::default();
    prepare_surface(&mut prepared, &cache);
    assert_eq!(prepared.rows(), geometry.rows);
    assert_eq!(prepared.columns(), geometry.columns);
    assert_eq!(prepared.generation(), Some(cache.generation));
    let renderer_ready_ns = elapsed_ns(committed_at);

    RawAttachment {
        stream,
        attachment_id: attached.attachment_id,
        reconnect_ns,
        renderer_ready_ns,
    }
}

fn cleanup_attachment(mode: LossMode, worker: &mut RuntimeWorker, mut attachment: RawAttachment) {
    worker.expect_cleanup_transition = true;
    match mode {
        LossMode::Graceful => {
            send_frame(
                &mut attachment.stream,
                MessageType::Detach,
                &Detach {
                    attachment_id: attachment.attachment_id,
                }
                .encode(),
            );
            let payload = read_until(&mut attachment.stream, MessageType::Detached as u16);
            let detached = Detached::decode(&payload).expect("Detached decode");
            assert_eq!(detached.attachment_id, attachment.attachment_id);
            let _ = attachment.stream.shutdown(Shutdown::Both);
        }
        LossMode::Abrupt => {
            let _ = attachment.stream.shutdown(Shutdown::Both);
        }
    }
    drop(attachment);
}
