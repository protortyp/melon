mod arg;
use anyhow::{anyhow, Result};
use melon_common::proto::Resources;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn parse_mbatch_comments(path: &str) -> Result<Resources> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut cpu_count: Option<u32> = None;
    let mut memory: Option<u64> = None;
    let mut time_limit_mins: Option<u32> = None;

    for line in reader.lines() {
        let line = line?;
        if line.starts_with("#MBATCH") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }
            match parts[1] {
                "-c" => cpu_count = parts[2].parse().ok(),
                "-m" => {
                    if let Some(mem_str) = parts[2].strip_suffix('G') {
                        memory = mem_str.parse::<u64>().ok().map(|m| m * 1024 * 1024 * 1024);
                    } else if let Some(mem_str) = parts[2].strip_suffix('M') {
                        memory = mem_str.parse::<u64>().ok().map(|m| m * 1024 * 1024);
                    } else {
                        // invalid or missing suffix
                        return Err(anyhow!("Unsupported memory suffix in {}", parts[2]));
                    }
                }
                "-t" => {
                    // Assuming time format is D-HH:MM
                    let time_parts: Vec<&str> = parts[2].split(&['-', ':']).collect();
                    if time_parts.len() == 3 {
                        let days: u32 = time_parts[0].parse()?;
                        let hours: u32 = time_parts[1].parse()?;
                        let minutes: u32 = time_parts[2].parse()?;
                        time_limit_mins = Some(days * 24 * 60 + hours * 60 + minutes);
                    }
                }
                _ => {}
            }
        }
    }

    if let (Some(cpu_count), Some(memory), Some(time)) = (cpu_count, memory, time_limit_mins) {
        Ok(Resources {
            cpu_count,
            memory,
            time,
        })
    } else {
        Err(anyhow!(
            "Missing required MBATCH parameters (cpu_count, memory, or time_limit)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "{}", content).unwrap();
        file
    }

    #[test]
    fn test_parse_valid_input() {
        let content = r#"
#!/bin/bash
#MBATCH -c 4
#MBATCH -m 8G
#MBATCH -t 1-12:30
echo "Hello, World!"
"#;
        let file = create_temp_file(content);
        let result = parse_mbatch_comments(file.path().to_str().unwrap()).unwrap();
        assert_eq!(result.cpu_count, 4);
        assert_eq!(result.memory, 8 * 1024 * 1024 * 1024);
        assert_eq!(result.time, 2190);
    }

    #[test]
    fn test_parse_memory_in_mb() {
        let content = "#MBATCH -c 2\n#MBATCH -m 512M\n#MBATCH -t 0-01:00";
        let file = create_temp_file(content);
        let result = parse_mbatch_comments(file.path().to_str().unwrap()).unwrap();
        assert_eq!(result.memory, 512 * 1024 * 1024);
    }

    #[test]
    fn test_parse_invalid_memory_suffix() {
        let content = "#MBATCH -c 2\n#MBATCH -m 512K\n#MBATCH -t 0-01:00";
        let file = create_temp_file(content);
        let result = parse_mbatch_comments(file.path().to_str().unwrap());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported memory suffix"));
    }

    #[test]
    fn test_parse_missing_parameters() {
        let content = "#MBATCH -c 2\n#MBATCH -m 4G";
        let file = create_temp_file(content);
        let result = parse_mbatch_comments(file.path().to_str().unwrap());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing required MBATCH parameters"));
    }

    #[test]
    fn test_parse_invalid_time_format() {
        let content = "#MBATCH -c 2\n#MBATCH -m 4G\n#MBATCH -t 1:30";
        let file = create_temp_file(content);
        let result = parse_mbatch_comments(file.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ignore_non_mbatch_lines() {
        let content = r#"
#!/bin/bash
# Some comment
#MBATCH -c 4
echo "Hello"
#MBATCH -m 8G
#MBATCH -t 0-02:00
"#;
        let file = create_temp_file(content);
        let result = parse_mbatch_comments(file.path().to_str().unwrap()).unwrap();
        assert_eq!(result.cpu_count, 4);
        assert_eq!(result.memory, 8 * 1024 * 1024 * 1024);
        assert_eq!(result.time, 120);
    }

    #[test]
    fn test_parse_invalid_numeric_values() {
        let content = "#MBATCH -c abc\n#MBATCH -m 4G\n#MBATCH -t 0-02:00";
        let file = create_temp_file(content);
        let result = parse_mbatch_comments(file.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_out_of_order_parameters() {
        let content = "#MBATCH -t 0-02:00\n#MBATCH -c 2\n#MBATCH -m 4G";
        let file = create_temp_file(content);
        let result = parse_mbatch_comments(file.path().to_str().unwrap()).unwrap();
        assert_eq!(result.cpu_count, 2);
        assert_eq!(result.memory, 4 * 1024 * 1024 * 1024);
        assert_eq!(result.time, 120);
    }
}
