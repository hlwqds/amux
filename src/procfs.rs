use anyhow::Result;
use std::time::Instant;

/// Resource usage stats for a single process, read from /proc/{pid}.
#[derive(Clone, Debug, Default)]
pub struct ProcessStats {
    pub cpu_user: u64,    // user ticks
    pub cpu_system: u64,  // system ticks
    pub mem_rss_kb: u64,  // resident set size in KB
    pub mem_virt_kb: u64, // virtual size in KB
    pub read_bytes: u64,  // IO read bytes
    pub write_bytes: u64, // IO write bytes
    pub threads: u64,
    pub uptime_secs: u64,
    // Computed fields
    pub cpu_percent: f64, // 0.0-100.0
    // Internal cache for delta computation
    pub prev_cpu_user: u64,
    pub prev_cpu_system: u64,
    pub prev_instant: Option<Instant>,
}

/// On Linux, CLK_TCK is typically 100 (USER_HZ).
/// We read it via libc::sysconf for correctness.
fn clock_ticks_per_sec() -> i64 {
    // SAFETY: sysconf is thread-safe and returns a constant on Linux.
    unsafe { libc::sysconf(libc::_SC_CLK_TCK) }
}

/// Page size in bytes. On x86_64 Linux this is always 4096.
fn page_size() -> i64 {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) }
}

/// Read process stats from /proc/{pid}/stat, /proc/{pid}/io, and /proc/uptime.
/// Aggregates the process tree (PID + all descendants) for accurate resource usage.
///
/// Returns default stats on non-Linux platforms.
#[allow(unused_variables)]
pub fn read_process_stats(pid: u32) -> Result<ProcessStats> {
    #[cfg(target_os = "linux")]
    {
        read_process_tree_stats_linux(pid)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(ProcessStats::default())
    }
}

#[cfg(target_os = "linux")]
fn read_process_stats_linux(pid: u32) -> Result<ProcessStats> {
    let stat_path = format!("/proc/{}/stat", pid);
    let stat_content = std::fs::read_to_string(&stat_path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {}", stat_path, e))?;

    // Field 14 (0-indexed 13): utime
    // Field 15 (0-indexed 14): stime
    // Field 20 (0-indexed 19): num_threads
    // Field 22 (0-indexed 21): starttime
    // Field 23 (0-indexed 22): vsize (bytes)
    // Field 24 (0-indexed 23): rss (pages)
    //
    // TRICKY: (comm) can contain spaces and parens, so find the LAST ')' first.
    let close_paren = stat_content
        .rfind(')')
        .ok_or_else(|| anyhow::anyhow!("malformed /proc/{}/stat: no closing paren", pid))?;
    let after_comm = &stat_content[close_paren + 1..]; // starts with " state ..."
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    // fields[0] = state, fields[1] = ppid, ...
    // We need fields indexed from after state:
    //   utime  = fields[11] (13 - 2 = 11, because fields[0] is state)
    //   stime  = fields[12]
    //   num_threads = fields[17]
    //   starttime = fields[19]
    //   vsize = fields[20]
    //   rss = fields[21]
    let utime: u64 = fields
        .get(11)
        .ok_or_else(|| anyhow::anyhow!("missing utime field"))?
        .parse()?;
    let stime: u64 = fields
        .get(12)
        .ok_or_else(|| anyhow::anyhow!("missing stime field"))?
        .parse()?;
    let num_threads: u64 = fields
        .get(17)
        .ok_or_else(|| anyhow::anyhow!("missing num_threads field"))?
        .parse()?;
    let starttime: u64 = fields
        .get(19)
        .ok_or_else(|| anyhow::anyhow!("missing starttime field"))?
        .parse()?;
    let vsize: u64 = fields
        .get(20)
        .ok_or_else(|| anyhow::anyhow!("missing vsize field"))?
        .parse()?;
    let rss_pages: u64 = fields
        .get(21)
        .ok_or_else(|| anyhow::anyhow!("missing rss field"))?
        .parse()?;

    let clk_tck = clock_ticks_per_sec().max(1) as u64;
    let pg_size = page_size().max(1) as u64;

    // Read /proc/uptime for system uptime
    let uptime_secs = std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .next()
                .and_then(|v| v.parse::<f64>().ok())
        })
        .unwrap_or(0.0);

    let process_uptime = if uptime_secs > 0.0 {
        (uptime_secs - starttime as f64 / clk_tck as f64).max(0.0) as u64
    } else {
        0
    };

    // Read /proc/{pid}/io
    let io_path = format!("/proc/{}/io", pid);
    let io_content = std::fs::read_to_string(&io_path).unwrap_or_default();
    let mut read_bytes: u64 = 0;
    let mut write_bytes: u64 = 0;
    for line in io_content.lines() {
        if let Some(val) = line.strip_prefix("read_bytes:") {
            read_bytes = val.trim().parse().unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("write_bytes:") {
            write_bytes = val.trim().parse().unwrap_or(0);
        }
    }

    Ok(ProcessStats {
        cpu_user: utime,
        cpu_system: stime,
        mem_rss_kb: rss_pages * pg_size / 1024,
        mem_virt_kb: vsize / 1024,
        read_bytes,
        write_bytes,
        threads: num_threads,
        uptime_secs: process_uptime,
        cpu_percent: 0.0,
        prev_cpu_user: 0,
        prev_cpu_system: 0,
        prev_instant: None,
    })
}

/// Collect all descendant PIDs by scanning /proc/*/stat for matching ppid.
#[cfg(target_os = "linux")]
fn collect_descendants(root_pid: u32) -> Vec<u32> {
    let mut result = vec![root_pid];
    let mut queue = std::collections::VecDeque::from([root_pid]);
    // Build ppid map: scan all /proc/{n}/stat entries
    let mut children_map: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if let Ok(pid) = name_str.parse::<u32>() {
                let stat_path = format!("/proc/{}/stat", pid);
                if let Ok(content) = std::fs::read_to_string(&stat_path) {
                    // Extract ppid: find last ')', then split, field[1] is ppid
                    if let Some(close) = content.rfind(')') {
                        let after = &content[close + 1..];
                        let fields: Vec<&str> = after.split_whitespace().collect();
                        if let Some(ppid_str) = fields.get(1)
                            && let Ok(ppid) = ppid_str.parse::<u32>()
                        {
                            children_map.entry(ppid).or_default().push(pid);
                        }
                    }
                }
            }
        }
    }
    // BFS from root
    while let Some(parent) = queue.pop_front() {
        if let Some(kids) = children_map.get(&parent) {
            for &child in kids {
                result.push(child);
                queue.push_back(child);
            }
        }
    }
    result
}

/// Read stats for an entire process tree (root PID + all descendants).
/// Aggregates CPU, memory, IO across all processes in the tree.
#[cfg(target_os = "linux")]
fn read_process_tree_stats_linux(root_pid: u32) -> Result<ProcessStats> {
    let pids = collect_descendants(root_pid);
    let mut aggregated = ProcessStats::default();
    let mut max_uptime: u64 = 0;
    for pid in &pids {
        if let Ok(s) = read_process_stats_linux(*pid) {
            aggregated.cpu_user = aggregated.cpu_user.saturating_add(s.cpu_user);
            aggregated.cpu_system = aggregated.cpu_system.saturating_add(s.cpu_system);
            aggregated.mem_rss_kb = aggregated.mem_rss_kb.saturating_add(s.mem_rss_kb);
            aggregated.mem_virt_kb = aggregated.mem_virt_kb.saturating_add(s.mem_virt_kb);
            aggregated.read_bytes = aggregated.read_bytes.saturating_add(s.read_bytes);
            aggregated.write_bytes = aggregated.write_bytes.saturating_add(s.write_bytes);
            aggregated.threads = aggregated.threads.saturating_add(s.threads);
            max_uptime = max_uptime.max(s.uptime_secs);
        }
    }
    aggregated.uptime_secs = max_uptime;
    Ok(aggregated)
}

/// Compute CPU percentage from previous measurement.
/// On first call (prev_instant is None), caches current values and returns 0.0.
pub fn compute_cpu_percent(stats: &mut ProcessStats) {
    let now = Instant::now();
    if let Some(prev) = stats.prev_instant {
        let elapsed = prev.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            let delta_user = stats.cpu_user.saturating_sub(stats.prev_cpu_user);
            let delta_system = stats.cpu_system.saturating_sub(stats.prev_cpu_system);
            let delta = delta_user + delta_system;
            let clk_tck = clock_ticks_per_sec().max(1) as f64;
            stats.cpu_percent = (delta as f64 / clk_tck) / elapsed * 100.0;
            // Clamp to reasonable range
            stats.cpu_percent = stats.cpu_percent.clamp(0.0, 100.0 * 128.0); // support multi-core
        }
    }
    stats.prev_cpu_user = stats.cpu_user;
    stats.prev_cpu_system = stats.cpu_system;
    stats.prev_instant = Some(now);
}

/// Format bytes into human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes < KB {
        format!("{}B", bytes)
    } else if bytes < MB {
        format!("{:.0}KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.0}MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0B");
        assert_eq!(format_bytes(512), "512B");
        assert_eq!(format_bytes(1023), "1023B");
        assert_eq!(format_bytes(1024), "1KB");
        assert_eq!(format_bytes(1536), "2KB");
        assert_eq!(format_bytes(1024 * 1024 - 1), "1024KB");
        assert_eq!(format_bytes(1024 * 1024), "1MB");
        assert_eq!(format_bytes(1024 * 1024 * 512), "512MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0GB");
        assert_eq!(
            format_bytes(1024 * 1024 * 1024 * 3 + 1024 * 1024 * 500),
            "3.5GB"
        );
    }

    #[test]
    fn test_compute_cpu_percent_first_call() {
        let mut stats = ProcessStats {
            cpu_user: 100,
            cpu_system: 50,
            ..Default::default()
        };
        compute_cpu_percent(&mut stats);
        // First call should return 0.0
        assert_eq!(stats.cpu_percent, 0.0);
        assert_eq!(stats.prev_cpu_user, 100);
        assert_eq!(stats.prev_cpu_system, 50);
        assert!(stats.prev_instant.is_some());
    }

    #[test]
    fn test_parse_proc_stat_mock() {
        // Mock /proc/PID/stat content: pid (command with spaces) S ...fields...
        // After last ')', fields start at state
        let line =
            "12345 (some command) S 1 2 3 4 5 6 7 8 9 10 11 13 14 15 16 17 18 19 20 21 22 23 24 25";
        let close_paren = line.rfind(')').unwrap();
        assert_eq!(close_paren, 19);
        let after = &line[close_paren + 1..];
        let fields: Vec<&str> = after.split_whitespace().collect();
        // fields[0] = "S" (state)
        // fields[11] should be utime, fields[12] stime
        assert_eq!(fields[0], "S");
        // In this mock, fields are "S 1 2 3 4 5 6 7 8 9 10 11 13 14 15 16 17 18 19 20 21 22 23 24 25"
        // index:        0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24
        assert_eq!(fields[11], "11"); // utime mock value
        assert_eq!(fields[12], "13"); // stime mock value
    }
}
