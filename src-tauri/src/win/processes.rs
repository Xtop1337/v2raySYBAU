use std::collections::BTreeSet;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

/// Executable names of running processes (for the per-app proxy picker), sorted, unique.
pub fn running_executables() -> Vec<String> {
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let names: BTreeSet<String> = system
        .processes()
        .values()
        .map(|p| p.name().to_string_lossy().to_string())
        .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("System") && !name.eq_ignore_ascii_case("Idle"))
        .collect();
    let mut list: Vec<String> = names.into_iter().collect();
    list.sort_by_key(|n| n.to_ascii_lowercase());
    list
}
