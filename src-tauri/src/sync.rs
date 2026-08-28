// git 同步模块：基于 git2 库实现 pull / commit / push
//
// 同步规则（单人使用场景）：
// 1. 先 commit 本地变更再 pull——pull 的 checkout 会重置工作区，未提交的写入会被丢弃
// 2. 本地无提交、或本地历史与远程无共同祖先（新机器/数据目录重建后首次同步）：
//    以远程为基准接管历史。tasks/categories 按 id 合并两边条目（同 id 任务取
//    updatedAt 较新者，本地先记的任务不会被覆盖）；settings.json 始终用本地版
//    （token/快捷键等是设备配置）；远程没有的本地文件（如 hosts.json）恢复保留
// 3. 正常合并（有共同祖先）时若冲突，以本地为准，下次 push 覆盖远程
use crate::storage::Settings;
use git2::{
    Cred, CredentialType, FetchOptions, PushOptions, RemoteCallbacks, Repository, ResetType,
};
use std::path::Path;

/// 同步结果：展示给用户的消息 + 是否拉到了远程新内容
/// （拉到内容时调用方需广播 data-changed，让前端从磁盘重载数据）
pub struct SyncOutcome {
    pub message: String,
    pub pulled: bool,
}

/// 完整同步流程：commit 本地变更 → pull → push
pub fn sync(data_dir: &Path, settings: &Settings) -> Result<SyncOutcome, String> {
    // 1. 打开或初始化仓库
    let repo = match Repository::open(data_dir) {
        Ok(r) => r,
        Err(_) => Repository::init(data_dir).map_err(|e| e.to_string())?,
    };

    // 2. 确保 remote origin 配置正确
    ensure_remote(&repo, settings)?;

    let configured = !settings.remote_url.is_empty();
    let mut msgs: Vec<String> = Vec::new();
    let mut pulled = false;

    // 3. commit 本地变更（必须先于 pull：pull 的 merge/checkout 会强制重置工作区，
    //    未提交的写入会被直接丢弃——曾导致 hosts.json 新数据被抹回旧内容）
    if has_changes(&repo)? {
        commit_all(&repo)?;
        msgs.push("已提交本地变更".into());
    }

    // 4. pull
    if configured {
        match pull(&repo, settings) {
            Ok(PullResult::UpToDate) => {}
            Ok(PullResult::Merged) => {
                pulled = true;
                msgs.push("已拉取远程更新".into());
            }
            Ok(PullResult::Adopted) => {
                pulled = true;
                msgs.push("本机与远程历史不相关，已按远程数据恢复任务记录".into());
            }
            Err(e) => msgs.push(format!("拉取失败：{e}")),
        }
    }

    // 5. push
    if configured {
        match push(&repo, settings) {
            Ok(()) => msgs.push("已推送到远程".into()),
            Err(e) => msgs.push(format!("推送失败：{e}")),
        }
    }

    let message = if msgs.is_empty() {
        "无变更".into()
    } else {
        msgs.join("；")
    };
    Ok(SyncOutcome { message, pulled })
}

fn ensure_remote(repo: &Repository, settings: &Settings) -> Result<(), String> {
    if settings.remote_url.is_empty() {
        return Ok(());
    }
    // 若已有 origin 则更新其 URL，否则新建
    match repo.find_remote("origin") {
        Ok(_) => repo
            .remote_set_url("origin", &settings.remote_url)
            .map_err(|e| e.to_string())?,
        Err(_) => {
            repo.remote("origin", &settings.remote_url)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ===== 认证 =====

fn remote_callbacks(settings: &Settings) -> RemoteCallbacks<'_> {
    let mut cb = RemoteCallbacks::new();
    let username = settings.username.clone();
    let token = settings.token.clone();
    cb.credentials(move |_url, username_from_url, _allowed: CredentialType| {
        let u = username_from_url.unwrap_or(&username);
        Cred::userpass_plaintext(u, &token)
    });
    cb
}

// ===== pull =====

enum PullResult {
    /// 远程无新内容
    UpToDate,
    /// 正常合并（含 fast-forward）
    Merged,
    /// 以远程为基准接管（本地空库或历史不相关）
    Adopted,
}

/// 拉取远程默认分支并合并
fn pull(repo: &Repository, settings: &Settings) -> Result<PullResult, String> {
    // fetch 所有分支到 refs/remotes/origin/*
    {
        let mut remote = repo.find_remote("origin").map_err(|e| e.to_string())?;
        let mut fo = FetchOptions::new();
        fo.remote_callbacks(remote_callbacks(settings));
        remote
            .fetch(
                &["+refs/heads/*:refs/remotes/origin/*"],
                Some(&mut fo),
                None,
            )
            .map_err(|e| e.to_string())?;
    }

    // 远程默认分支（兼容 master/main），以其最新提交为合并目标
    let branch = remote_default_branch(repo, settings)?;
    let remote_ref = repo
        .find_reference(&format!("refs/remotes/origin/{branch}"))
        .map_err(|e| e.to_string())?;
    let fetch_commit = repo
        .reference_to_annotated_commit(&remote_ref)
        .map_err(|e| e.to_string())?;
    let remote_tip = repo
        .find_commit(fetch_commit.id())
        .map_err(|e| e.to_string())?;

    let local_head = match repo.head() {
        Ok(h) => Some(h.peel_to_commit().map_err(|e| e.to_string())?),
        Err(_) => None,
    };

    // 本地无任何提交（全新仓库）：直接以远程为基准建分支
    let Some(head) = local_head else {
        adopt_remote(repo, &branch, &remote_tip, None)?;
        return Ok(PullResult::Adopted);
    };

    if head.id() == remote_tip.id() {
        return Ok(PullResult::UpToDate);
    }

    // merge_base 找不到共同祖先时返回 Err(NotFound)，即视为历史不相关
    if repo.merge_base(head.id(), remote_tip.id()).is_ok() {
        // 有共同祖先：正常合并
        merge_remote(repo, &fetch_commit, &[&head, &remote_tip])
    } else {
        // 历史不相关（新机器/数据目录重建后首次同步）：以远程为基准接管。
        // 旧逻辑此处走普通合并，冲突时「以本地为准」把远程任务记录永久丢弃，
        // 且历史不相关导致 push 被远程拒绝，表现为永远拉不下来
        adopt_remote(repo, &branch, &remote_tip, Some(&head))?;
        Ok(PullResult::Adopted)
    }
}

/// 询问远程服务器默认分支名（master/main 均可）；失败时按 master → main 回退
fn remote_default_branch(repo: &Repository, settings: &Settings) -> Result<String, String> {
    let from_server = || -> Result<String, String> {
        let mut remote = repo.find_remote("origin").map_err(|e| e.to_string())?;
        let cbs = remote_callbacks(settings);
        remote
            .connect_auth(git2::Direction::Fetch, Some(cbs), None)
            .map_err(|e| e.to_string())?;
        let branch = remote.default_branch().map_err(|e| e.to_string());
        let _ = remote.disconnect();
        let full = branch.map_err(|e| e.to_string())?;
        Ok(full
            .as_str()
            .unwrap_or("refs/heads/master")
            .trim_start_matches("refs/heads/")
            .to_string())
    }();

    match from_server {
        Ok(b) => Ok(b),
        Err(_) => {
            for fallback in ["master", "main"] {
                if repo
                    .find_reference(&format!("refs/remotes/origin/{fallback}"))
                    .is_ok()
                {
                    return Ok(fallback.into());
                }
            }
            Err("无法确定远程默认分支".into())
        }
    }
}

/// 正常合并路径：优先 fast-forward，冲突时以本地为准（下次 push 覆盖远程）
fn merge_remote(
    repo: &Repository,
    fetch_commit: &git2::AnnotatedCommit<'_>,
    parents: &[&git2::Commit<'_>],
) -> Result<PullResult, String> {
    let analysis = repo
        .merge_analysis(&[fetch_commit])
        .map_err(|e| e.to_string())?;

    if analysis.0.is_up_to_date() {
        return Ok(PullResult::UpToDate);
    }

    if analysis.0.is_fast_forward() {
        let refname = format!("refs/heads/{}", current_branch_name(repo));
        repo.find_reference(&refname)
            .map_err(|e| e.to_string())?
            .set_target(fetch_commit.id(), "pull: fast-forward")
            .map_err(|e| e.to_string())?;
        repo.set_head(&refname).map_err(|e| e.to_string())?;
        checkout_force(repo)?;
        return Ok(PullResult::Merged);
    }

    if analysis.0.is_normal() {
        repo.merge(&[fetch_commit], None, None)
            .map_err(|e| e.to_string())?;

        if repo.index().map_err(|e| e.to_string())?.has_conflicts() {
            // 冲突：以本地为准，放弃本次合并（单人场景，下次 push 覆盖远程）
            repo.cleanup_state().map_err(|e| e.to_string())?;
            let head = repo
                .head()
                .map_err(|e| e.to_string())?
                .peel_to_commit()
                .map_err(|e| e.to_string())?;
            repo.reset(&head.as_object(), ResetType::Hard, None)
                .map_err(|e| e.to_string())?;
        } else {
            // 无冲突：生成双亲合并提交（只写单亲会截断本地历史，远程会拒绝 push）
            let sig = signature(repo)?;
            let tree_id = repo
                .index()
                .map_err(|e| e.to_string())?
                .write_tree()
                .map_err(|e| e.to_string())?;
            let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;
            repo.commit(Some("HEAD"), &sig, &sig, "merge remote", &tree, parents)
                .map_err(|e| e.to_string())?;
            checkout_force(repo)?;
        }
        return Ok(PullResult::Merged);
    }

    Ok(PullResult::UpToDate)
}

/// 以远程为基准重建本地分支（本地空库，或本地历史与远程不相关）。
/// local_head 提供时：恢复本机 settings.json 与远程没有的本地文件（如 hosts.json），
/// 并把这些差异提交到新基线上，保证后续 push 是 fast-forward。
fn adopt_remote(
    repo: &Repository,
    branch: &str,
    remote_tip: &git2::Commit<'_>,
    local_head: Option<&git2::Commit<'_>>,
) -> Result<(), String> {
    // 本地分支强制指向远程最新提交（不存在则新建）
    let refname = format!("refs/heads/{branch}");
    match repo.find_reference(&refname) {
        Ok(mut r) => {
            r.set_target(remote_tip.id(), "sync: adopt remote history")
                .map_err(|e| e.to_string())?;
        }
        Err(_) => {
            repo.branch(branch, remote_tip, true)
                .map_err(|e| e.to_string())?;
        }
    }
    repo.set_head(&format!("refs/heads/{branch}"))
        .map_err(|e| e.to_string())?;
    repo.reset(remote_tip.as_object(), ResetType::Hard, None)
        .map_err(|e| e.to_string())?;

    if let Some(local) = local_head {
        // 数据文件（tasks/categories）按 id 合并两边条目，本地先记的任务不能被覆盖丢失
        merge_local_data(repo, local)?;
        restore_local_files(repo, local, remote_tip)?;
        if has_changes(repo)? {
            commit_all(repo)?;
        }
    }
    Ok(())
}

/// 接管远程历史时，把本地数据条目按 id 并入远程版数据文件：
/// - tasks.json：按 id 并集，同 id 取 updatedAt 较新的一方（远程无此文件则整体写入本地版）
/// - categories.json：按 id 并集，同 id 取本机版（用户当前使用的名称）
fn merge_local_data(repo: &Repository, local: &git2::Commit<'_>) -> Result<(), String> {
    let local_tree = local.tree().map_err(|e| e.to_string())?;
    merge_array_file(repo, &local_tree, "tasks.json", |local, remote| {
        let newer = |v: &serde_json::Value| -> String {
            v.get("updatedAt")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string()
        };
        newer(local) >= newer(remote)
    })?;
    merge_array_file(repo, &local_tree, "categories.json", |_local, _remote| true)?;
    Ok(())
}

/// 数组型 JSON 数据文件的内容级合并：远程版（reset 后的工作区）打底，
/// 本地版（旧本地提交）按 "id" 字段并入。任一侧缺失或解析失败时保守处理：
/// 本地无此文件跳过；远程无此文件（或不可解析）则直接写入本地版。
fn merge_array_file(
    repo: &Repository,
    local_tree: &git2::Tree<'_>,
    file: &str,
    local_wins: impl Fn(&serde_json::Value, &serde_json::Value) -> bool,
) -> Result<(), String> {
    use serde_json::Value;

    let id_of = |v: &Value| -> Option<String> {
        v.get("id").and_then(|i| i.as_str()).map(String::from)
    };

    // 本地版内容（来自旧本地提交）
    let Some(entry) = local_tree.get_name(file) else {
        return Ok(());
    };
    let Ok(obj) = entry.to_object(repo) else {
        return Ok(());
    };
    let Some(blob) = obj.as_blob() else {
        return Ok(());
    };
    let Ok(Value::Array(local_items)) = serde_json::from_slice::<Value>(blob.content()) else {
        return Ok(());
    };

    let Some(workdir) = repo.workdir().map(|p| p.to_path_buf()) else {
        return Err("仓库没有工作区".into());
    };
    let path = workdir.join(file);

    // 远程版内容（reset --hard 后的工作区即远程版）
    let remote_items: Vec<Value> = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| match v {
            Value::Array(a) => Some(a),
            _ => None,
        })
        .unwrap_or_default();

    // 远程打底保持顺序，本地条目按 id 并入：独有则追加，同 id 按规则择一
    let mut map: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for item in remote_items {
        let Some(id) = id_of(&item) else {
            continue;
        };
        if !map.contains_key(&id) {
            order.push(id.clone());
        }
        map.insert(id, item);
    }
    for item in local_items {
        let Some(id) = id_of(&item) else {
            continue;
        };
        let take_local = match map.get(&id) {
            Some(remote) => local_wins(&item, remote),
            None => {
                order.push(id.clone());
                true
            }
        };
        if take_local {
            map.insert(id, item);
        }
    }

    let merged: Vec<Value> = order
        .into_iter()
        .filter_map(|id| map.remove(&id))
        .collect();
    let out = serde_json::to_string_pretty(&Value::Array(merged)).map_err(|e| e.to_string())?;
    std::fs::write(&path, out).map_err(|e| e.to_string())?;
    Ok(())
}

/// 接管远程历史时，从旧本地提交恢复设备相关文件：
/// - 远程没有的文件（如 hosts.json）→ 恢复本地版
/// - settings.json → 始终本地版（token/快捷键/自启动是本机配置，不能被远程覆盖）
/// - 其余数据文件（tasks/categories）已由 merge_local_data 按 id 合并，此处跳过
fn restore_local_files(
    repo: &Repository,
    local: &git2::Commit<'_>,
    remote_tip: &git2::Commit<'_>,
) -> Result<(), String> {
    let local_tree = local.tree().map_err(|e| e.to_string())?;
    let remote_tree = remote_tip.tree().map_err(|e| e.to_string())?;
    let Some(workdir) = repo.workdir().map(|p| p.to_path_buf()) else {
        return Err("仓库没有工作区".into());
    };

    for entry in local_tree.iter() {
        let Some(name) = entry.name() else { continue };
        if remote_tree.get_name(name).is_some() && name != "settings.json" {
            continue;
        }
        let Ok(obj) = entry.to_object(repo) else { continue };
        let Some(blob) = obj.as_blob() else { continue };
        std::fs::write(workdir.join(name), blob.content()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ===== commit =====

fn has_changes(repo: &Repository) -> Result<bool, String> {
    let statuses = repo.statuses(None).map_err(|e| e.to_string())?;
    Ok(!statuses.is_empty())
}

fn commit_all(repo: &Repository) -> Result<(), String> {
    let mut index = repo.index().map_err(|e| e.to_string())?;
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .map_err(|e| e.to_string())?;
    index.write().map_err(|e| e.to_string())?;

    let tree_id = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;
    let sig = signature(repo)?;
    let msg = format!("update {}", timestamp());

    let parent = match repo.head() {
        Ok(h) => Some(h.peel_to_commit().map_err(|e| e.to_string())?),
        Err(_) => None,
    };

    if let Some(p) = parent {
        repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree, &[&p])
            .map_err(|e| e.to_string())?;
    } else {
        repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree, &[])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ===== push =====

fn push(repo: &Repository, settings: &Settings) -> Result<(), String> {
    let mut remote = repo.find_remote("origin").map_err(|e| e.to_string())?;
    let branch = current_branch_name(repo);
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    let mut po = PushOptions::new();
    po.remote_callbacks(remote_callbacks(settings));
    remote
        .push(&[refspec.as_str()], Some(&mut po))
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ===== 工具 =====

fn current_branch_name(repo: &Repository) -> String {
    repo.head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()))
        .unwrap_or_else(|| "master".into())
}

fn checkout_force(repo: &Repository) -> Result<(), String> {
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .map_err(|e| e.to_string())
}

fn signature(repo: &Repository) -> Result<git2::Signature<'_>, String> {
    match repo.signature() {
        Ok(s) => Ok(s),
        Err(_) => git2::Signature::now("todo-app", "todo@local").map_err(|e| e.to_string()),
    }
}

fn timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}
