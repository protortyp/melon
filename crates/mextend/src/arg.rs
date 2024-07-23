use clap::Parser;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// API Endpoint
    #[arg(
        short = 'a',
        long = "api_endpoint",
        default_value = "http://[::1]:8080"
    )]
    pub api_endpoint: String,

    /// The job id
    #[arg(short = 'j', long = "job")]
    pub job: u64,

    /// Time extension in D-HH-MM format
    #[arg(short = 't', long = "time", value_parser = parse_time_extension)]
    pub extension: Duration,
}

fn parse_time_extension(arg: &str) -> Result<Duration, String> {
    let parts: Vec<&str> = arg.split('-').collect();
    if parts.len() != 3 {
        return Err("Time extension must be in D-HH-MM format".to_string());
    }

    let days = parts[0].parse::<u64>().map_err(|_| "Invalid day format")?;
    let hours = parts[1].parse::<u64>().map_err(|_| "Invalid hour format")?;
    let minutes = parts[2]
        .parse::<u64>()
        .map_err(|_| "Invalid minute format")?;

    if hours >= 24 {
        return Err("Hours must be less than 24".to_string());
    }
    if minutes >= 60 {
        return Err("Minutes must be less than 60".to_string());
    }

    Ok(Duration::from_secs(
        days * 24 * 60 * 60 + hours * 60 * 60 + minutes * 60,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_valid_input() {
        let result = parse_time_extension("2-12-30");
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Duration::from_secs(2 * 24 * 60 * 60 + 12 * 60 * 60 + 30 * 60)
        );
    }

    #[test]
    fn test_invalid_format() {
        let result = parse_time_extension("2-12");
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "Time extension must be in D-HH-MM format".to_string()
        );
    }

    #[test]
    fn test_invalid_day_format() {
        let result = parse_time_extension("x-12-30");
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "Invalid day format".to_string());
    }

    #[test]
    fn test_invalid_hour_format() {
        let result = parse_time_extension("2-xx-30");
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "Invalid hour format".to_string());
    }

    #[test]
    fn test_invalid_minute_format() {
        let result = parse_time_extension("2-12-xx");
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), "Invalid minute format".to_string());
    }

    #[test]
    fn test_hours_greater_than_24() {
        let result = parse_time_extension("2-25-30");
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "Hours must be less than 24".to_string()
        );
    }

    #[test]
    fn test_minutes_greater_than_60() {
        let result = parse_time_extension("2-12-61");
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "Minutes must be less than 60".to_string()
        );
    }

    #[test]
    fn test_edge_case_24_hours() {
        let result = parse_time_extension("1-23-59");
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Duration::from_secs(24 * 60 * 60 + 23 * 60 * 60 + 59 * 60)
        );
    }

    #[test]
    fn test_zero_time() {
        let result = parse_time_extension("0-00-00");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Duration::from_secs(0));
    }
}
