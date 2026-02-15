use serde::Serialize;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Serialize)]
pub struct GitStatus {
    pub branch: String,
    pub is_clean: bool,
    pub modified: Vec<String>,
    pub untracked: Vec<String>,
}

/// Entrada del historial de commits (log).
#[derive(Debug, Clone, Serialize)]
pub struct CommitInfo {
    pub id_short: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

/// Archivo cambiado en un commit (para detalle del historial).
#[derive(Debug, Clone, Serialize)]
pub struct CommitFileChange {
    pub path: String,
    pub status: String, // "added", "modified", "deleted"
}

pub struct GitRepo(git2::Repository);

#[derive(Debug, Error)]
pub enum GitError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    #[error("No repository found")]
    NoRepo,
}

pub fn open_repo(path: &Path) -> Result<GitRepo, GitError> {
    let repo = git2::Repository::open(path).map_err(|e| {
        if e.code() == git2::ErrorCode::NotFound {
            GitError::NoRepo
        } else {
            GitError::Git(e)
        }
    })?;
    Ok(GitRepo(repo))
}

/// Clona un repositorio por URL en la ruta indicada.
pub fn clone_repo(url: &str, path: &Path) -> Result<GitRepo, GitError> {
    let repo = git2::Repository::clone(url, path)?;
    Ok(GitRepo(repo))
}

pub fn status(repo: &GitRepo) -> Result<GitStatus, GitError> {
    let r = &repo.0;
    let branch = current_branch(repo)?.unwrap_or_else(|| "HEAD".to_string());
    let mut modified = Vec::new();
    let mut untracked = Vec::new();
    let mut status_opts = git2::StatusOptions::new();
    status_opts.include_untracked(true);
    status_opts.exclude_submodules(true);
    for entry in &r.statuses(Some(&mut status_opts))? {
        let path = entry.path().unwrap_or("").to_string();
        match entry.status() {
            s if s.is_index_new() || s.is_index_modified() || s.is_wt_modified() || s.is_wt_new() => {
                if !modified.contains(&path) {
                    modified.push(path);
                }
            }
            s if s.is_wt_new() && entry.status() == git2::Status::WT_NEW => {
                if !untracked.contains(&path) {
                    untracked.push(path);
                }
            }
            _ => {}
        }
    }
    for entry in &r.statuses(Some(&mut status_opts))? {
        let path = entry.path().unwrap_or("").to_string();
        if entry.status() == git2::Status::WT_NEW {
            if !untracked.contains(&path) {
                untracked.push(path);
            }
        }
    }
    let is_clean = modified.is_empty() && untracked.is_empty();
    Ok(GitStatus {
        branch,
        is_clean,
        modified,
        untracked,
    })
}

pub fn current_branch(repo: &GitRepo) -> Result<Option<String>, GitError> {
    let r = &repo.0;
    let head = match r.head() {
        Ok(h) => h,
        Err(_) => return Ok(None),
    };
    Ok(head.shorthand().map(String::from))
}

/// Lista los nombres de las ramas locales.
pub fn branches(repo: &GitRepo) -> Result<Vec<String>, GitError> {
    let r = &repo.0;
    let mut names = Vec::new();
    for name in r.branches(Some(git2::BranchType::Local))? {
        let (branch, _) = name?;
        if let Some(s) = branch.name()? {
            names.push(s.to_string());
        }
    }
    names.sort();
    Ok(names)
}

/// Cambia a la rama indicada (git checkout).
pub fn checkout_branch(repo: &GitRepo, branch_name: &str) -> Result<(), GitError> {
    let r = &repo.0;
    let refname = format!("refs/heads/{}", branch_name);
    let _obj = r.revparse_single(&refname).map_err(|_| GitError::NoRepo)?;
    r.set_head(&refname)?;
    let mut opts = git2::build::CheckoutBuilder::new();
    opts.force();
    r.checkout_head(Some(&mut opts))?;
    Ok(())
}

pub fn commit(repo: &GitRepo, message: &str, paths: &[&Path]) -> Result<(), GitError> {
    let r = &repo.0;
    let mut index = r.index()?;
    if paths.is_empty() {
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    } else {
        for p in paths {
            index.add_path(p)?;
        }
    }
    index.write()?;
    let tree_id = index.write_tree()?;
    let tree = r.find_tree(tree_id)?;
    let sig = r.signature()?;
    let parent = r.head().ok().and_then(|h| h.peel_to_commit().ok());
    let mut parents = Vec::new();
    if let Some(p) = parent {
        parents.push(p);
    }
    let _commit_id = r.commit(
        Some("HEAD"),
        &sig,
        &sig,
        message,
        &tree,
        parents.iter().collect::<Vec<_>>().as_slice(),
    )?;
    Ok(())
}

pub fn pull(repo: &GitRepo) -> Result<String, GitError> {
    let r = &repo.0;
    let branch = current_branch(repo)?.unwrap_or_else(|| "main".to_string());
    let mut remote = r.find_remote("origin").or_else(|_| r.remote_anonymous("origin"))?;
    remote.fetch(&[] as &[&str], None, None)?;
    let fetch_head = r.find_reference("FETCH_HEAD")?;
    let fetch_commit = r.reference_to_annotated_commit(&fetch_head)?;
    let (analysis, _) = r.merge_analysis(&[&fetch_commit])?;
    if analysis.is_up_to_date() {
        return Ok("Already up to date.".to_string());
    }
    let refname = format!("refs/heads/{}", branch);
    if analysis.is_fast_forward() {
        let mut reference = r.find_reference(&refname)?;
        reference.set_target(fetch_commit.id(), "Fast-Forward")?;
        r.set_head(&refname)?;
        let mut opts = git2::build::CheckoutBuilder::new();
        opts.force();
        r.checkout_head(Some(&mut opts))?;
        Ok("Pull (fast-forward) completed.".to_string())
    } else if analysis.is_normal() {
        r.merge(&[&fetch_commit], None, None)?;
        Ok("Pull (merge) completed.".to_string())
    } else {
        Err(git2::Error::from_str("Merge not possible").into())
    }
}

pub fn push(repo: &GitRepo) -> Result<String, GitError> {
    let r = &repo.0;
    let mut remote = r.find_remote("origin").map_err(|_| GitError::NoRepo)?;
    let branch = current_branch(repo)?.unwrap_or_else(|| "main".to_string());
    let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);
    remote.push(&[&refspec], None)?;
    Ok("Push completed".to_string())
}

/// Historial de commits (log) de la rama actual.
pub fn log(repo: &GitRepo, max_count: usize) -> Result<Vec<CommitInfo>, GitError> {
    let r = &repo.0;
    let mut revwalk = r.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME)?;
    let mut out = Vec::new();
    for oid in revwalk.take(max_count) {
        let oid = oid?;
        let c = r.find_commit(oid)?;
        let msg = c.message().unwrap_or("").trim().lines().next().unwrap_or("").to_string();
        let author = c.author().name().unwrap_or("").to_string();
        let time = c.time();
        let date = format_timestamp(time.seconds());
        out.push(CommitInfo {
            id_short: oid.to_string()[..7.min(oid.to_string().len())].to_string(),
            message: msg,
            author,
            date,
        });
    }
    Ok(out)
}

fn format_timestamp(secs: i64) -> String {
    use chrono::TimeZone;
    chrono::Utc
        .timestamp_opt(secs, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| format!("{}", secs))
}

/// Archivos cambiados en un commit (respecto al padre).
pub fn commit_changes(repo: &GitRepo, commit_id_short: &str) -> Result<Vec<CommitFileChange>, GitError> {
    let r = &repo.0;
    let prefix = commit_id_short.trim();
    if prefix.len() < 7 {
        return Err(git2::Error::from_str("commit id too short").into());
    }
    let oid = {
        let mut revwalk = r.revwalk()?;
        revwalk.push_head()?;
        revwalk
            .find(|o| o.as_ref().map(|id| id.to_string().starts_with(prefix)).unwrap_or(false))
            .ok_or_else(|| git2::Error::from_str("commit not found"))??
    };
    let commit = r.find_commit(oid)?;
    let tree = commit.tree()?;
    let parent_tree = match commit.parent(0) {
        Ok(p) => p.tree()?,
        Err(_) => {
            return Ok(vec![CommitFileChange {
                path: "(commit inicial)".to_string(),
                status: "initial".to_string(),
            }]);
        }
    };
    let mut out = Vec::new();
    let diff = r.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;
    diff.foreach(
        &mut |delta, _progress| {
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let status = match delta.status() {
                git2::Delta::Added => "added",
                git2::Delta::Deleted => "deleted",
                git2::Delta::Modified => "modified",
                git2::Delta::Renamed => "renamed",
                git2::Delta::Copied => "copied",
                _ => "changed",
            };
            out.push(CommitFileChange {
                path,
                status: status.to_string(),
            });
            true
        },
        None,
        None,
        None,
    )?;
    Ok(out)
}

/// Lista los nombres de las ramas remotas (origin/xxx → xxx). Ejecutar fetch antes para tener refs actualizados.
pub fn branches_remote(repo: &GitRepo) -> Result<Vec<String>, GitError> {
    let r = &repo.0;
    let mut names = Vec::new();
    for name in r.branches(Some(git2::BranchType::Remote))? {
        let (branch, _) = name?;
        if let Ok(Some(s)) = branch.name() {
            let s = s.to_string();
            if let Some(short) = s.split('/').nth(1) {
                if !short.is_empty() && !names.contains(&short.to_string()) {
                    names.push(short.to_string());
                }
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Actualiza las referencias remotas (fetch origin).
pub fn fetch(repo: &GitRepo) -> Result<(), GitError> {
    let r = &repo.0;
    let mut remote = r.find_remote("origin").map_err(|_| GitError::NoRepo)?;
    remote.fetch(&[] as &[&str], None, None)?;
    Ok(())
}

/// Cambia a una rama remota: si existe local la hace checkout; si no, crea la rama local desde origin/name y hace checkout.
pub fn checkout_remote_branch(repo: &GitRepo, branch_name: &str) -> Result<(), GitError> {
    let r = &repo.0;
    let local_ref = format!("refs/heads/{}", branch_name);
    let remote_ref = format!("refs/remotes/origin/{}", branch_name);
    if r.find_reference(&local_ref).is_ok() {
        return checkout_branch(repo, branch_name);
    }
    let remote_ref_obj = r.find_reference(&remote_ref)?;
    let commit = remote_ref_obj.peel_to_commit()?;
    let _ = r.branch(branch_name, &commit, false)?;
    r.set_head(&local_ref)?;
    let mut opts = git2::build::CheckoutBuilder::new();
    opts.force();
    r.checkout_head(Some(&mut opts))?;
    Ok(())
}

/// Guarda los cambios actuales en el stash (git stash).
pub fn stash(repo: &mut GitRepo) -> Result<(), GitError> {
    let r = &mut repo.0;
    let sig = r.signature()?;
    r.stash_save(&sig, "", None)?;
    Ok(())
}

/// Aplica el último stash y lo elimina de la lista (git stash pop).
pub fn stash_pop(repo: &mut GitRepo) -> Result<(), GitError> {
    let r = &mut repo.0;
    r.stash_apply(0, None)?;
    r.stash_drop(0)?;
    Ok(())
}
