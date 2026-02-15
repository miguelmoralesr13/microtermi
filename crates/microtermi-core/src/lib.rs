pub mod discovery;
pub mod env;
pub mod git;
pub mod gitlab;
pub mod scripts;

pub use discovery::{scan_projects, Project};
pub use env::{load_env, save_env, Environment};
pub use git::{
    branches, branches_remote, checkout_branch, checkout_remote_branch, clone_repo, commit,
    commit_changes, fetch, log, open_repo, push, pull, stash, stash_pop, status, CommitFileChange,
    CommitInfo, GitRepo, GitStatus,
};
pub use gitlab::{clone_url_with_token, list_branches, list_projects, GitLabBranch, GitLabError, GitLabProject};
pub use scripts::{
    detect_package_manager, run_script, run_script_captured, run_scripts, PackageManager,
    ScriptRunMode,
};
