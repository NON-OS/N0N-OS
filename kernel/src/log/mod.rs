//! NØNOS Logging Subsystem

pub mod logger;

pub use logger::{
    Logger, LogLevel, Severity,
    init as init_logger,
    try_get_logger,
    log, log_info, log_warn, log_err, log_dbg, log_fatal,
    enter_panic_mode
};
