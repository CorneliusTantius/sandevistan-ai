use std::{
    fs::{self, File},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn output_with_timeout(
    label: &str,
    command: &mut Command,
    timeout: Duration,
) -> Result<Output, String> {
    let (stdout_path, stderr_path) = output_paths(label);
    let stdout_file = File::create(&stdout_path)
        .map_err(|error| format!("{label} stdout temp create failed: {error}"))?;
    let stderr_file = File::create(&stderr_path)
        .map_err(|error| format!("{label} stderr temp create failed: {error}"))?;

    let mut child = command
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(|error| format!("{label} failed: {error}"))?;

    let start = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("{label} wait failed: {error}"))?
        {
            let stdout = read_and_remove(&stdout_path);
            let stderr = read_and_remove(&stderr_path);
            return Ok(Output {
                status,
                stdout,
                stderr,
            });
        }

        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&stdout_path);
            let _ = fs::remove_file(&stderr_path);
            return Err(format!("{label} timed out after {}s", timeout.as_secs()));
        }

        thread::sleep(Duration::from_millis(25));
    }
}

fn output_paths(label: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let safe_label = label
        .chars()
        .map(|value| match value {
            '/' | '\\' | ' ' => '_',
            other => other,
        })
        .collect::<String>();
    let base = std::env::temp_dir().join(format!(
        "sandevistan-{safe_label}-{}-{unique}",
        std::process::id()
    ));
    (base.with_extension("out"), base.with_extension("err"))
}

fn read_and_remove(path: &std::path::Path) -> Vec<u8> {
    let content = fs::read(path).unwrap_or_default();
    let _ = fs::remove_file(path);
    content
}
