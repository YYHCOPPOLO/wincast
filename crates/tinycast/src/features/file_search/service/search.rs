//! Windows Search OLE DB (`SystemIndex`) with a capped directory walk fallback.

use std::path::{Path, PathBuf};

use tinycast_pure::file_search::policy::keep_home_child;
use tinycast_pure::file_search::query::{
    cap_candidates, is_excluded_path, like_clauses, matches_filename, rank, CANDIDATE_LIMIT,
};
use tinycast_pure::file_search::{FileSearchHit, FileSearchPolicy};
use windows::core::{w, Interface, GUID, PCWSTR};
use windows::Win32::Foundation::MAX_PATH;
use windows::Win32::Storage::FileSystem::{
    FindClose, FindFirstFileW, FindNextFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_HIDDEN,
    FILE_ATTRIBUTE_REPARSE_POINT, WIN32_FIND_DATAW,
};
use windows::Win32::System::Com::{
    CLSIDFromProgID, CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Search::{
    IAccessor, ICommandText, IDBCreateCommand, IDBCreateSession, IDBInitialize, IRowset, DBBINDING,
    HACCESSOR,
};

const DBGUID_DEFAULT: GUID = GUID::from_u128(0xC8B521FB_5CF3_11CE_ADE5_00AA0044773D);
const DBPART_VALUE: u32 = 0x1;
const DBTYPE_WSTR: u16 = 8;
const DBACCESSOR_ROWDATA: u32 = 0x1;

#[derive(Debug)]
pub struct SearchError;

pub fn search(query: &str, policy: &FileSearchPolicy) -> Result<Vec<FileSearchHit>, SearchError> {
    if tinycast_pure::file_search::tokens(query).is_empty() {
        return Ok(Vec::new());
    }
    let ole = ole_search(query, policy);
    if use_walk_fallback(&ole) {
        Ok(rank(walk_search(query, policy), query, &policy.ignore))
    } else {
        Ok(rank(ole.unwrap_or_default(), query, &policy.ignore))
    }
}

/// Empty OLE success is catalog-useless: fall through to the capped walk.
pub fn use_walk_fallback(ole: &Result<Vec<FileSearchHit>, SearchError>) -> bool {
    match ole {
        Ok(hits) => hits.is_empty(),
        Err(_) => true,
    }
}

fn ole_search(query: &str, policy: &FileSearchPolicy) -> Result<Vec<FileSearchHit>, SearchError> {
    let scopes = resolved_directories(policy);
    if scopes.is_empty() && !policy.includes_home {
        return Ok(Vec::new());
    }
    let Some(where_clause) = like_clauses(query) else {
        return Ok(Vec::new());
    };
    let mut scope_sql = Vec::new();
    for dir in &scopes {
        let native = dir.to_string_lossy().replace('/', "\\");
        let escaped = native.replace('\'', "''");
        scope_sql.push(format!("SCOPE='file:{escaped}'"));
    }
    if scope_sql.is_empty() {
        return Err(SearchError);
    }
    let mut extras = String::new();
    for glob in policy.ignore.search_name_exclusions() {
        extras.push_str(&format!(
            " AND NOT System.FileName LIKE '{}'",
            glob.replace('\'', "''")
        ));
    }
    let sql = format!(
        "SELECT TOP {CANDIDATE_LIMIT} System.ItemPathDisplay FROM SystemIndex WHERE ({}) AND ({}){extras}",
        scope_sql.join(" OR "),
        where_clause
    );
    let paths = ole_select_paths(&sql)?;
    let mut hits = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if policy.includes_home {
        for hit in home_root_matches(query, policy) {
            if seen.insert(hit.path.clone()) {
                hits.push(hit);
            }
        }
    }
    for path in paths {
        if cap_candidates(hits.len() + 1) == hits.len() {
            break;
        }
        if is_excluded_path(&path, &policy.ignore) {
            continue;
        }
        let is_dir = std::fs::metadata(&path)
            .map(|m| m.is_dir())
            .unwrap_or(false);
        let hit = FileSearchHit::from_path(&path, is_dir, &policy.home);
        if seen.insert(hit.path.clone()) {
            hits.push(hit);
        }
    }
    Ok(hits)
}

fn ole_select_paths(sql: &str) -> Result<Vec<String>, SearchError> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let clsid = CLSIDFromProgID(w!("Search.CollatorDSO.1")).map_err(|_| SearchError)?;
        let init: IDBInitialize =
            CoCreateInstance(&clsid, None, CLSCTX_INPROC_SERVER).map_err(|_| SearchError)?;
        init.Initialize().map_err(|_| SearchError)?;
        let sessions: IDBCreateSession = init.cast().map_err(|_| SearchError)?;
        let session = sessions
            .CreateSession(None, &IDBCreateCommand::IID)
            .map_err(|_| SearchError)?;
        let commands: IDBCreateCommand = session.cast().map_err(|_| SearchError)?;
        let command_unk = commands
            .CreateCommand(None, &ICommandText::IID)
            .map_err(|_| SearchError)?;
        let command: ICommandText = command_unk.cast().map_err(|_| SearchError)?;
        let sql_wide: Vec<u16> = sql.encode_utf16().chain(std::iter::once(0)).collect();
        command
            .SetCommandText(&DBGUID_DEFAULT, PCWSTR(sql_wide.as_ptr()))
            .map_err(|_| SearchError)?;
        let mut unk = None;
        command
            .Execute(None, &IRowset::IID, None, None, Some(&mut unk))
            .map_err(|_| SearchError)?;
        let rowset: IRowset = unk.ok_or(SearchError)?.cast().map_err(|_| SearchError)?;
        read_rowset_paths(&rowset)
    }
}

unsafe fn read_rowset_paths(rowset: &IRowset) -> Result<Vec<String>, SearchError> {
    let accessor: IAccessor = rowset.cast().map_err(|_| SearchError)?;
    const BUF: usize = (MAX_PATH as usize + 8) * 2;
    let mut binding = DBBINDING::default();
    binding.iOrdinal = 1;
    binding.obValue = 0;
    binding.dwPart = DBPART_VALUE;
    binding.cbMaxLen = BUF;
    binding.wType = DBTYPE_WSTR;
    let mut haccessor = HACCESSOR::default();
    accessor
        .CreateAccessor(DBACCESSOR_ROWDATA, 1, &binding, BUF, &mut haccessor, None)
        .map_err(|_| SearchError)?;
    let mut paths = Vec::new();
    loop {
        let mut rows_got = 0usize;
        let mut rows_ptr: *mut usize = std::ptr::null_mut();
        let hr = get_next_rows(rowset, 1, &mut rows_got, &mut rows_ptr);
        if hr.is_err() || rows_got == 0 || rows_ptr.is_null() {
            break;
        }
        let hrow = *rows_ptr;
        let mut buf = vec![0u16; MAX_PATH as usize + 8];
        if rowset
            .GetData(hrow, haccessor, buf.as_mut_ptr().cast())
            .is_ok()
        {
            let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
            let path = String::from_utf16_lossy(&buf[..len]);
            if !path.is_empty() {
                paths.push(path);
            }
        }
        let _ = rowset.ReleaseRows(
            rows_got,
            rows_ptr,
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        if rows_got < 1 {
            break;
        }
    }
    let _ = accessor.ReleaseAccessor(haccessor, None);
    Ok(paths)
}

/// Real OLE arity: `cRows` plus `pcRowsObtained` / `prghRows`.
unsafe fn get_next_rows(
    rowset: &IRowset,
    crows: isize,
    obtained: &mut usize,
    prghrows: *mut *mut usize,
) -> windows::core::HRESULT {
    let this = windows::core::Interface::as_raw(rowset);
    let vtbl = *(this as *const *const usize);
    // IUnknown (3) + AddRefRows + GetData + GetNextRows
    let slot = *vtbl.add(5);
    let f: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        usize,
        isize,
        isize,
        *mut usize,
        *mut *mut usize,
    ) -> i32 = std::mem::transmute(slot);
    windows::core::HRESULT(f(this, 0, 0, crows, obtained, prghrows))
}

fn walk_search(query: &str, policy: &FileSearchPolicy) -> Vec<FileSearchHit> {
    let mut hits = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if policy.includes_home {
        for hit in home_root_matches(query, policy) {
            if seen.insert(hit.path.clone()) {
                hits.push(hit);
            }
        }
    }
    let mut stack = resolved_directories(policy);
    while let Some(dir) = stack.pop() {
        if cap_candidates(hits.len() + 1) == hits.len() {
            break;
        }
        let children = list_dir(&dir);
        for child in children {
            let path = child.path.to_string_lossy().replace('\\', "/");
            if is_excluded_path(&path, &policy.ignore) {
                continue;
            }
            if child.is_dir && !child.is_reparse {
                stack.push(child.path);
            }
            if !matches_filename(&child.name, query) {
                continue;
            }
            if !seen.insert(path.clone()) {
                continue;
            }
            hits.push(FileSearchHit::from_path(&path, child.is_dir, &policy.home));
            if cap_candidates(hits.len() + 1) == hits.len() {
                break;
            }
        }
    }
    hits
}

struct Dirent {
    path: PathBuf,
    name: String,
    is_dir: bool,
    is_reparse: bool,
    hidden: bool,
}

fn resolved_directories(policy: &FileSearchPolicy) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for root in &policy.direct_roots {
        dirs.push(PathBuf::from(root.replace('/', "\\")));
    }
    if policy.includes_home {
        let home = PathBuf::from(policy.home.replace('/', "\\"));
        for child in list_dir(&home) {
            if !keep_home_child(&child.name, child.hidden, false) {
                continue;
            }
            if child.is_dir && !child.is_reparse {
                dirs.push(child.path);
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    dirs.into_iter()
        .filter(|d| seen.insert(d.to_string_lossy().to_lowercase()))
        .collect()
}

fn home_root_matches(query: &str, policy: &FileSearchPolicy) -> Vec<FileSearchHit> {
    let home = PathBuf::from(policy.home.replace('/', "\\"));
    list_dir(&home)
        .into_iter()
        .filter(|c| keep_home_child(&c.name, c.hidden, false))
        .filter(|c| matches_filename(&c.name, query))
        .map(|c| {
            FileSearchHit::from_path(
                &c.path.to_string_lossy().replace('\\', "/"),
                c.is_dir,
                &policy.home,
            )
        })
        .filter(|hit| !is_excluded_path(&hit.path, &policy.ignore))
        .collect()
}

fn list_dir(dir: &Path) -> Vec<Dirent> {
    let mut out = Vec::new();
    let pattern = dir.join("*");
    let wide: Vec<u16> = pattern
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut data = WIN32_FIND_DATAW::default();
    unsafe {
        let Ok(handle) = FindFirstFileW(PCWSTR(wide.as_ptr()), &mut data) else {
            return out;
        };
        loop {
            if let Some(child) = dirent_from(dir, &data) {
                out.push(child);
            }
            if FindNextFileW(handle, &mut data).is_err() {
                break;
            }
        }
        let _ = FindClose(handle);
    }
    out
}

fn dirent_from(dir: &Path, data: &WIN32_FIND_DATAW) -> Option<Dirent> {
    let name = {
        let len = data
            .cFileName
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(data.cFileName.len());
        String::from_utf16_lossy(&data.cFileName[..len])
    };
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    let attrs = data.dwFileAttributes;
    let hidden = attrs & FILE_ATTRIBUTE_HIDDEN.0 != 0 || name.starts_with('.');
    if hidden {
        return None;
    }
    Some(Dirent {
        path: dir.join(&name),
        name,
        is_dir: attrs & FILE_ATTRIBUTE_DIRECTORY.0 != 0,
        is_reparse: attrs & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0,
        hidden,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn walk_respects_caps_and_and_tokens() {
        let root = std::env::temp_dir().join(format!(
            "tinycast-fs-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("node_modules")).unwrap();
        fs::write(root.join("annual report.txt"), b"x").unwrap();
        fs::write(root.join("annual notes.txt"), b"x").unwrap();
        fs::write(root.join("node_modules").join("annual report.js"), b"x").unwrap();
        let policy = FileSearchPolicy::new(
            &[root.to_string_lossy().replace('\\', "/")],
            &[],
            "C:/Users/test",
        );
        let hits = walk_search("annual report", &policy);
        let names: Vec<_> = hits.iter().map(|h| h.name.as_str()).collect();
        assert!(names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("annual report.txt")));
        assert!(!names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("annual notes.txt")));
        assert!(!hits.iter().any(|h| h.path.contains("node_modules")));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_ole_success_walks() {
        assert!(use_walk_fallback(&Err(SearchError)));
        assert!(use_walk_fallback(&Ok(Vec::new())));
        let hit = FileSearchHit::from_path("C:/a.txt", false, "C:/Users/test");
        assert!(!use_walk_fallback(&Ok(vec![hit])));
    }
}
