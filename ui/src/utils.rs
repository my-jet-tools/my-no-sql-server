pub fn format_bytes(n: f64) -> String {
    let mut n = n;
    if n < 1024.0 {
        return format!("{:.0}b", n);
    }
    n /= 1024.0;
    if n < 1024.0 {
        return format!("{:.2}Kb", n);
    }
    n /= 1024.0;
    if n < 1024.0 {
        return format!("{:.2}Mb", n);
    }
    n /= 1024.0;
    format!("{:.2}Gb", n)
}

/// Throughput that scales with magnitude (b/s, Kb/s, Mb/s, Gb/s) so even a few
/// bytes per second stay visible instead of rounding away to "0.00 MB/s".
pub fn format_bytes_per_sec(bytes_per_sec: f64) -> String {
    format!("{}/s", format_bytes(bytes_per_sec))
}

pub fn format_duration(micros: f64) -> String {
    if micros == 0.0 {
        return "0".to_string();
    }
    if micros < 1_000.0 {
        return format!("{}µs", micros as i64);
    }
    if micros < 1_000_000.0 {
        return format!("{:.3}ms", micros / 1_000.0);
    }
    format!("{:.3}s", micros / 1_000_000.0)
}

pub fn format_unix_microseconds(value: i64) -> String {
    if value <= 0 {
        return "—".to_string();
    }

    let secs = value / 1_000_000;
    let micros = (value % 1_000_000) as u32;

    let (year, month, day, h, m, s) = unix_to_ymdhms(secs);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
        year, month, day, h, m, s, micros
    )
}

fn unix_to_ymdhms(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let seconds_in_day = secs.rem_euclid(86_400);

    let h = (seconds_in_day / 3600) as u32;
    let m = ((seconds_in_day % 3600) / 60) as u32;
    let s = (seconds_in_day % 60) as u32;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m_civil = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m_civil <= 2 { y + 1 } else { y };

    (year as i32, m_civil as u32, d as u32, h, m, s)
}
