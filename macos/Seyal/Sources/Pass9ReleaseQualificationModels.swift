import Foundation

extension Pass9ReleaseQualification {
  struct Artifact: Encodable {
    let schema: String
    let measurementSource: String
    let commit: String
    let recovery: RecoveryContract
    let cohorts: [Cohort]
    let pass8: Pass8Attribution?
    let topologyNote: String

    enum CodingKeys: String, CodingKey {
      case schema
      case measurementSource = "measurement_source"
      case commit
      case recovery
      case cohorts
      case pass8
      case topologyNote = "topology_note"
    }
  }

  struct RecoveryContract: Encodable {
    let attempts: Int
    let retryDelaysMs: [Int]
    let deadlineMs: Int
    let launchesPerEpisodeMax: Int

    enum CodingKeys: String, CodingKey {
      case attempts
      case retryDelaysMs = "retry_delays_ms"
      case deadlineMs = "deadline_ms"
      case launchesPerEpisodeMax = "launches_per_episode_max"
    }
  }

  struct Pass8Attribution: Encodable {
    let pairedDeltaPercent: Double
    let gate: String
    let cohorts: Int
    let rootCauseExplanation: String?

    enum CodingKeys: String, CodingKey {
      case pairedDeltaPercent = "paired_delta_percent"
      case gate
      case cohorts
      case rootCauseExplanation = "root_cause_explanation"
    }
  }

  struct Cohort: Encodable {
    let mode: String
    let geometry: String
    let cohort: Int
    let cycles: Int
    let reconnectP99Us: Double
    let cleanupP99Us: Double
    let preparedSurfaceP99Us: Double
    let nativeReadyP99Us: Double
    let detachedCpuSamplesPercent: [Double]
    let detachedCpuP95Percent: Double
    let runtimeRssDeltaKib: Int
    let clientRssDeltaKib: Int
    let attachmentsBaseline: Int
    let attachmentsFinal: Int
    let controllersBaseline: Int
    let controllersFinal: Int
    let runtimeFdsBaseline: Int
    let runtimeFdsFinal: Int
    let clientFdsBaseline: Int
    let clientFdsFinal: Int
    let runtimeThreadsBaseline: Int
    let runtimeThreadsFinal: Int
    let clientThreadsBaseline: Int
    let clientThreadsFinal: Int
    let socketsBaseline: Int
    let socketsFinal: Int
    let rendererSurfacesBaseline: Int
    let rendererSurfacesFinal: Int
    let rendererGpuResourcesBaseline: Int
    let rendererGpuResourcesFinal: Int
    let pendingResyncBaseline: Int
    let pendingResyncFinal: Int
    let retryTimersBaseline: Int
    let retryTimersFinal: Int
    let runtimeAllocatorInUseKibBaseline: Int
    let runtimeAllocatorInUseKibFinal: Int
    let clientAllocatorInUseKibBaseline: Int
    let clientAllocatorInUseKibFinal: Int
    let clientAllocatorDeltaClassification: String

    enum CodingKeys: String, CodingKey {
      case mode, geometry, cohort, cycles
      case reconnectP99Us = "reconnect_p99_us"
      case cleanupP99Us = "cleanup_p99_us"
      case preparedSurfaceP99Us = "prepared_surface_p99_us"
      case nativeReadyP99Us = "native_ready_p99_us"
      case detachedCpuSamplesPercent = "detached_cpu_samples_percent"
      case detachedCpuP95Percent = "detached_cpu_p95_percent"
      case runtimeRssDeltaKib = "runtime_rss_delta_kib"
      case clientRssDeltaKib = "client_rss_delta_kib"
      case attachmentsBaseline = "attachments_baseline"
      case attachmentsFinal = "attachments_final"
      case controllersBaseline = "controllers_baseline"
      case controllersFinal = "controllers_final"
      case runtimeFdsBaseline = "runtime_fds_baseline"
      case runtimeFdsFinal = "runtime_fds_final"
      case clientFdsBaseline = "client_fds_baseline"
      case clientFdsFinal = "client_fds_final"
      case runtimeThreadsBaseline = "runtime_threads_baseline"
      case runtimeThreadsFinal = "runtime_threads_final"
      case clientThreadsBaseline = "client_threads_baseline"
      case clientThreadsFinal = "client_threads_final"
      case socketsBaseline = "sockets_baseline"
      case socketsFinal = "sockets_final"
      case rendererSurfacesBaseline = "renderer_surfaces_baseline"
      case rendererSurfacesFinal = "renderer_surfaces_final"
      case rendererGpuResourcesBaseline = "renderer_gpu_resources_baseline"
      case rendererGpuResourcesFinal = "renderer_gpu_resources_final"
      case pendingResyncBaseline = "pending_resync_baseline"
      case pendingResyncFinal = "pending_resync_final"
      case retryTimersBaseline = "retry_timers_baseline"
      case retryTimersFinal = "retry_timers_final"
      case runtimeAllocatorInUseKibBaseline = "runtime_allocator_in_use_kib_baseline"
      case runtimeAllocatorInUseKibFinal = "runtime_allocator_in_use_kib_final"
      case clientAllocatorInUseKibBaseline = "client_allocator_in_use_kib_baseline"
      case clientAllocatorInUseKibFinal = "client_allocator_in_use_kib_final"
      case clientAllocatorDeltaClassification = "client_allocator_delta_classification"
    }
  }}
