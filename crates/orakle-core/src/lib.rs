pub mod adapters;
pub mod downloads;
pub mod error;
pub mod profile;
pub mod proxy;
pub mod pulse;
pub mod shield;
pub mod storage;

pub use adapters::{
    compile_to_wk_content_rule_list, platform_capability_matrix,
    sync_rules_file_for_webkit_extension, AdapterCapabilities, ContentRuleCompilationResult,
};
pub use downloads::{
    classify_filename_risk, download_resumable_to_path, finalize_partial_download,
    partial_path_for, resolve_safe_destination, sanitize_filename, DownloadControl,
    DownloadSafetyRisk, DownloadStatusSnapshot, DL_STATE_ACTIVE, DL_STATE_CANCELLED,
    DL_STATE_COMPLETED, DL_STATE_ERROR, DL_STATE_PAUSED,
};
pub use error::{OrakleError, Result};
pub use profile::{PrivateSession, ProfileContext, ProfileRegistry, ProfileSummary};
pub use proxy::{
    is_forbidden_private_ip, is_host_loopback, sanitize_url_for_log, validate_navigation_url,
    validate_navigation_with_origin, NavigationOrigin, ProxyConfig, ProxyDecision, ProxyEvent,
    ProxyHealth, ProxyTelemetry, SecureLoopbackProxy, ShieldLogEntry,
};
pub use pulse::{NetworkPulse, PhasePercentiles, PulseBenchmarkStats, PulseResult};
pub use shield::{BenchmarkStats, ShieldDecision, ShieldEngine};
pub use storage::{
    BookmarkEntry, DomainPermission, DownloadRecord, HistoryEntry, NoteEntry, ProfileRecord,
    SessionRecord, Space, StorageEngine, TabGroup, TabRecord,
};
