//! Tool specification constructors shared by Codex runtime crates.
//!
//! Keeping pure Responses API tool schemas here prevents schema-only changes
//! from invalidating the much larger `codex-core` crate.

pub mod agent_jobs;
pub mod apply_patch;
pub mod code_mode_execute;
pub mod code_mode_wait;
pub mod goal;
pub mod hosted;
pub mod list_available_plugins_to_install;
pub mod mcp_resource;
pub mod multi_agents;
pub mod plan;
pub mod request_plugin_install;
pub mod request_user_input;
pub mod shell;
pub mod test_sync;
pub mod tool_search;
pub mod view_image;
