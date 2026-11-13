pub mod adapters;
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
pub use error::{OrakleError, Result};
pub use profile::{PrivateSession, ProfileContext};
pub use proxy::{
    is_forbidden_private_ip, validate_navigation_url, ProxyConfig, ProxyDecision, ProxyEvent,
    ProxyTelemetry, SecureLoopbackProxy,
};
pub use pulse::{NetworkPulse, PhasePercentiles, PulseBenchmarkStats, PulseResult};
pub use shield::{BenchmarkStats, ShieldDecision, ShieldEngine};
pub use storage::{
    BookmarkEntry, DownloadRecord, HistoryEntry, NoteEntry, ProfileRecord, SessionRecord, Space,
    StorageEngine, TabGroup, TabRecord,
};
