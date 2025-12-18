use clap::Parser;
#[cfg(target_os = "linux")]
use inotify::{EventMask, Inotify, WatchMask};
use litra::{Device, DeviceError, DeviceHandle, Litra};
#[cfg(target_os = "macos")]
use log::debug;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
#[cfg(target_os = "macos")]
use std::process::Stdio;
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tokio::io::{AsyncBufReadExt, BufReader};
#[cfg(target_os = "macos")]
use tokio::process::Command;
use tokio::sync::Mutex;

/// Configuration structure for YAML file deserialization.
/// Field names use underscores to match YAML convention (e.g. serial_number).
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    serial_number: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    device_path: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    device_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    require_device: Option<bool>,

    #[cfg(target_os = "linux")]
    #[serde(skip_serializing_if = "Option::is_none")]
    video_device: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    delay: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    verbose: Option<bool>,
}

/// Automatically turn your Logitech Litra device on when your webcam turns on, and off when your webcam turns off (macOS and Linux only).
#[derive(Debug, Parser)]
#[clap(name = "litra-autotoggle", version)]
struct Cli {
    #[clap(
        long,
        short = 'c',
        help = "Path to a YAML configuration file. Configuration values can be specified in the file with underscored names (e.g. serial_number). Command line arguments take precedence over config file values."
    )]
    config_file: Option<PathBuf>,

    #[clap(
        long,
        short,
        help = "Specify the device to target by its serial number. By default, all devices are targeted."
    )]
    serial_number: Option<String>,

    #[clap(
        long,
        short = 'p',
        help = "Specify the device to target by its path (useful for devices that don't show a serial number). By default, all devices are targeted."
    )]
    device_path: Option<String>,

    #[clap(
        long,
        short = 'y',
        help = "Specify the device to target by its type (`glow`, `beam` or `beam_lx`). By default, all devices are targeted."
    )]
    device_type: Option<String>,

    #[clap(
        long,
        short,
        action,
        help = "Exit with an error if no Litra device is found. By default, the program will run and listen for events even if no Litra device is found, but do nothing. With this option set, the program will exit whenever it looks for a Litra device and none is found."
    )]
    require_device: bool,

    #[cfg(target_os = "linux")]
    #[clap(
        long,
        short = 'd',
        help = "The path of the video device to monitor (e.g. `/dev/video0`) (Linux only). By default, all devices are monitored."
    )]
    video_device: Option<String>,

    #[clap(
        long,
        short = 't',
        default_value = "1500",
        help = "The delay in milliseconds between detecting a webcam event and toggling the Litra. When your webcam turns on or off, multiple events may be generated in quick succession. Setting a delay allows the program to wait for all events before taking action, avoiding flickering."
    )]
    delay: u64,

    #[clap(long, short, action, help = "Output detailed log messages")]
    verbose: bool,
}

fn check_device_filters<'a>(
    _context: &'a Litra,
    _serial_number: Option<&'a str>,
    device_path: Option<&'a str>,
    device_type: Option<&'a str>,
) -> impl Fn(&Device) -> bool + 'a {
    move |device| {
        // Check device path if specified
        if let Some(path) = device_path {
            return device.device_path() == path;
        }

        // Check device type if specified
        if let Some(expected_type) = device_type {
            let device_type_str = match device.device_type() {
                litra::DeviceType::LitraGlow => "glow",
                litra::DeviceType::LitraBeam => "beam",
                litra::DeviceType::LitraBeamLX => "beam_lx",
            };
            return device_type_str == expected_type;
        }

        // If a serial number is specified, we'll filter by it after opening the device
        // since serial numbers are only accessible after opening
        true
    }
}

#[derive(Debug)]
enum CliError {
    DeviceError(DeviceError),
    IoError(std::io::Error),
    NoDevicesFound,
    DeviceNotFound(String),
    MultipleFiltersSpecified,
    ConfigFileError(String),
    InvalidDeviceType(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::DeviceError(error) => error.fmt(f),
            CliError::IoError(error) => write!(f, "Input/output error: {error}"),
            CliError::NoDevicesFound => write!(f, "No Litra devices found"),
            CliError::DeviceNotFound(serial_number) => write!(
                f,
                "Litra device with serial number {serial_number} not found"
            ),
            CliError::MultipleFiltersSpecified => write!(
                f,
                "Only one filter (--serial-number, --device-path, or --device-type) can be specified at a time."
            ),
            CliError::ConfigFileError(error) => write!(f, "Configuration file error: {error}"),
            CliError::InvalidDeviceType(device_type) => write!(
                f,
                "Invalid device type '{device_type}'. Must be one of: glow, beam, beam_lx"
            ),
        }
    }
}

impl From<DeviceError> for CliError {
    fn from(error: DeviceError) -> Self {
        CliError::DeviceError(error)
    }
}

impl From<std::io::Error> for CliError {
    fn from(error: std::io::Error) -> Self {
        CliError::IoError(error)
    }
}

type CliResult = Result<(), CliError>;

/// Validates that only one filter is specified
fn validate_single_filter(
    serial_number: Option<&str>,
    device_path: Option<&str>,
    device_type: Option<&str>,
) -> Result<(), CliError> {
    let filter_count = [
        serial_number.is_some(),
        device_path.is_some(),
        device_type.is_some(),
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    if filter_count > 1 {
        Err(CliError::MultipleFiltersSpecified)
    } else {
        Ok(())
    }
}

/// Validates that device_type is one of the allowed values
fn validate_device_type(device_type: &str) -> Result<(), CliError> {
    match device_type {
        "glow" | "beam" | "beam_lx" => Ok(()),
        _ => Err(CliError::InvalidDeviceType(device_type.to_string())),
    }
}

/// Loads and validates the configuration from a YAML file
fn load_config_file(config_path: &PathBuf) -> Result<Config, CliError> {
    // Read the file
    let contents = fs::read_to_string(config_path)
        .map_err(|e| CliError::ConfigFileError(format!("Failed to read config file: {}", e)))?;

    // Parse YAML
    let config: Config = serde_yaml::from_str(&contents)
        .map_err(|e| CliError::ConfigFileError(format!("Failed to parse YAML: {}", e)))?;

    // Validate device_type if specified
    if let Some(ref device_type) = config.device_type {
        validate_device_type(device_type)?;
    }

    // Validate that only one filter is specified in config
    validate_single_filter(
        config.serial_number.as_deref(),
        config.device_path.as_deref(),
        config.device_type.as_deref(),
    )?;

    Ok(config)
}

/// Merges CLI arguments with config file values.
/// CLI arguments take precedence over config file values.
fn merge_config_with_cli(mut cli: Cli) -> Result<Cli, CliError> {
    if let Some(config_path) = &cli.config_file {
        let config = load_config_file(config_path)?;

        // Merge values - CLI takes precedence
        if cli.serial_number.is_none() {
            cli.serial_number = config.serial_number;
        }
        if cli.device_path.is_none() {
            cli.device_path = config.device_path;
        }
        if cli.device_type.is_none() {
            cli.device_type = config.device_type;
        }
        if !cli.require_device {
            cli.require_device = config.require_device.unwrap_or(false);
        }
        #[cfg(target_os = "linux")]
        {
            if cli.video_device.is_none() {
                cli.video_device = config.video_device;
            }
        }
        // Only use config delay if CLI has the default value (1500)
        if cli.delay == 1500 && config.delay.is_some() {
            cli.delay = config.delay.unwrap();
        }
        if !cli.verbose {
            cli.verbose = config.verbose.unwrap_or(false);
        }
    }

    // Validate device_type if specified via CLI or config
    if let Some(ref device_type) = cli.device_type {
        validate_device_type(device_type)?;
    }

    Ok(cli)
}

fn get_all_supported_devices(
    context: &mut Litra,
    serial_number: Option<&str>,
    device_path: Option<&str>,
    device_type: Option<&str>,
    require_device: bool,
) -> Result<Vec<DeviceHandle>, CliError> {
    // Validate that only one filter is used
    validate_single_filter(serial_number, device_path, device_type)?;

    {
        context.refresh_connected_devices()?;
    }

    // Filter by various criteria
    let potential_devices: Vec<Device> = context
        .get_connected_devices()
        .filter(check_device_filters(
            context,
            serial_number,
            device_path,
            device_type,
        ))
        .collect();

    // If we need to filter by serial, open devices and check
    let handles: Vec<DeviceHandle> = if let Some(serial) = serial_number {
        let mut handles = Vec::new();
        for device in potential_devices {
            if let Ok(handle) = device.open(context) {
                if let Ok(Some(actual_serial)) = handle.serial_number() {
                    if actual_serial == serial {
                        handles.push(handle);
                    }
                }
            }
        }
        handles
    } else {
        // No serial filter, include all devices that matched the other filters
        potential_devices
            .into_iter()
            .filter_map(|dev| dev.open(context).ok())
            .collect()
    };

    if handles.is_empty() && require_device {
        if let Some(serial_number) = serial_number {
            Err(CliError::DeviceNotFound(serial_number.to_string()))
        } else {
            Err(CliError::NoDevicesFound)
        }
    } else {
        Ok(handles)
    }
}

fn turn_on_all_supported_devices_and_log(
    context: &mut Litra,
    serial_number: Option<&str>,
    device_path: Option<&str>,
    device_type: Option<&str>,
    require_device: bool,
) -> Result<(), CliError> {
    let device_handles = get_all_supported_devices(
        context,
        serial_number,
        device_path,
        device_type,
        require_device,
    )?;

    if device_handles.is_empty() {
        print_device_not_found_log(serial_number);
    } else {
        for device_handle in device_handles {
            info!(
                "Turning on {} device (serial number: {})",
                device_handle.device_type(),
                get_serial_number_with_fallback(&device_handle)
            );

            // Ignore errors for individual devices when targeting multiple
            if let Err(e) = device_handle.set_on(true) {
                warn!(
                    "Failed to turn on {} device (serial number: {}): {}",
                    device_handle.device_type(),
                    get_serial_number_with_fallback(&device_handle),
                    e
                );
            }
        }
    }

    Ok(())
}

fn turn_off_all_supported_devices_and_log(
    context: &mut Litra,
    serial_number: Option<&str>,
    device_path: Option<&str>,
    device_type: Option<&str>,
    require_device: bool,
) -> Result<(), CliError> {
    let device_handles = get_all_supported_devices(
        context,
        serial_number,
        device_path,
        device_type,
        require_device,
    )?;

    if device_handles.is_empty() {
        print_device_not_found_log(serial_number);
    } else {
        for device_handle in device_handles {
            info!(
                "Turning off {} device (serial number: {})",
                device_handle.device_type(),
                get_serial_number_with_fallback(&device_handle)
            );

            // Ignore errors for individual devices when targeting multiple
            if let Err(e) = device_handle.set_on(false) {
                warn!(
                    "Failed to turn off {} device (serial number: {}): {}",
                    device_handle.device_type(),
                    get_serial_number_with_fallback(&device_handle),
                    e
                );
            }
        }
    }

    Ok(())
}

fn print_device_not_found_log(serial_number: Option<&str>) {
    if serial_number.is_some() {
        warn!(
            "Litra device with serial number {} not found",
            serial_number.unwrap()
        );
    } else {
        warn!("No Litra devices found");
    }
}

fn get_serial_number_with_fallback(device_handle: &DeviceHandle) -> String {
    match device_handle.serial_number().unwrap() {
        Some(serial_number) => serial_number.to_string(),
        None => "-".to_string(),
    }
}

#[cfg(target_os = "macos")]
async fn handle_autotoggle_command(
    serial_number: Option<&str>,
    device_path: Option<&str>,
    device_type: Option<&str>,
    require_device: bool,
    delay: u64,
) -> CliResult {
    // Wrap context in Arc<Mutex<>> to enable sharing across tasks
    let context = Arc::new(Mutex::new(Litra::new()?));

    // Use context inside an async block with locking
    {
        let mut context_lock = context.lock().await;
        let device_handles = get_all_supported_devices(
            &mut context_lock,
            serial_number,
            device_path,
            device_type,
            require_device,
        )?;
        if device_handles.is_empty() {
            print_device_not_found_log(serial_number);
        } else {
            for device_handle in device_handles {
                info!(
                    "Found {} device (serial number: {})",
                    device_handle.device_type(),
                    get_serial_number_with_fallback(&device_handle)
                );
            }
        }
    }

    info!("Starting `log` process to listen for video device events...");

    let mut child = Command::new("log")
        .arg("stream")
        .arg("--predicate")
        .arg("subsystem == \"com.apple.cmio\" AND (eventMessage CONTAINS \"AVCaptureSession_Tundra startRunning\" || eventMessage CONTAINS \"AVCaptureSession_Tundra stopRunning\")")
        .stdout(Stdio::piped())
        .spawn()?;

    let stdout = child
        .stdout
        .take()
        .expect("Failed to start `log` process to listen for video device events");
    let mut reader = BufReader::new(stdout).lines();

    info!("Listening for video device events...");

    // Add variables for throttling
    let mut pending_action: Option<tokio::task::JoinHandle<()>> = None;
    let desired_state = std::sync::Arc::new(tokio::sync::Mutex::new(None));

    while let Some(log_line) = reader
        .next_line()
        .await
        .expect("Failed to read log line from `log` process when listening for video device events")
    {
        if !log_line.starts_with("Filtering the log data") {
            debug!("Log line: {log_line}");

            // Update desired state based on the event
            if log_line.contains("AVCaptureSession_Tundra startRunning") {
                info!("Detected that a video device has been turned on.");

                let mut state = desired_state.lock().await;
                *state = Some(true);
            } else if log_line.contains("AVCaptureSession_Tundra stopRunning") {
                info!("Detected that a video device has been turned off.");

                let mut state = desired_state.lock().await;
                *state = Some(false);
            }

            // Cancel any pending action
            if let Some(handle) = pending_action.take() {
                handle.abort();
            }

            // Clone variables for the async task
            let desired_state_clone = desired_state.clone();
            let context_clone = context.clone();
            let serial_number_clone = serial_number.map(|s| s.to_string());
            let device_path_clone = device_path.map(|s| s.to_string());
            let device_type_clone = device_type.map(|s| s.to_string());

            // Start a new delayed action
            pending_action = Some(tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

                let state = {
                    let mut state = desired_state_clone.lock().await;
                    state.take()
                };

                if let Some(state) = state {
                    let mut context_lock = context_clone.lock().await;
                    if state {
                        info!("Attempting to turn on Litra device(s)...");
                        let _ = turn_on_all_supported_devices_and_log(
                            &mut context_lock,
                            serial_number_clone.as_deref(),
                            device_path_clone.as_deref(),
                            device_type_clone.as_deref(),
                            require_device,
                        );
                    } else {
                        info!("Attempting to turn off Litra device(s)...");
                        let _ = turn_off_all_supported_devices_and_log(
                            &mut context_lock,
                            serial_number_clone.as_deref(),
                            device_path_clone.as_deref(),
                            device_type_clone.as_deref(),
                            require_device,
                        );
                    }
                }
            }));
        }
    }

    let status = child.wait().await.expect(
        "Something went wrong with the `log` process when listening for video device events",
    );

    Err(CliError::IoError(std::io::Error::other(format!(
        "`log` process exited unexpectedly when listening for video device events - {status}"
    ))))
}

#[cfg(target_os = "linux")]
async fn handle_autotoggle_command(
    serial_number: Option<&str>,
    device_path: Option<&str>,
    device_type: Option<&str>,
    require_device: bool,
    video_device: Option<&str>,
    delay: u64,
) -> CliResult {
    // Wrap context in Arc<Mutex<>> to enable sharing across tasks
    let context = Arc::new(Mutex::new(Litra::new()?));

    // Use context inside an async block with locking
    {
        let mut context_lock = context.lock().await;
        let device_handles = get_all_supported_devices(
            &mut context_lock,
            serial_number,
            device_path,
            device_type,
            require_device,
        )?;
        if device_handles.is_empty() {
            print_device_not_found_log(serial_number);
        } else {
            for device_handle in device_handles {
                info!(
                    "Found {} device (serial number: {})",
                    device_handle.device_type(),
                    get_serial_number_with_fallback(&device_handle)
                );
            }
        }
    }

    // Path to watch for video device events
    let watch_path = video_device.unwrap_or("/dev");

    // Extract video device name from path, or use "video" as default
    let video_file_prefix = video_device
        .and_then(|p| p.split('/').next_back())
        .unwrap_or("video");

    let mut inotify = Inotify::init()?;
    match inotify
        .watches()
        .add(watch_path, WatchMask::OPEN | WatchMask::CLOSE)
    {
        Ok(_) => info!("Watching {}", watch_path),
        Err(e) => error!("Failed to watch {}: {}", watch_path, e),
    }

    // Add variables for throttling similar to macOS
    let mut pending_action: Option<tokio::task::JoinHandle<()>> = None;
    let desired_state = std::sync::Arc::new(tokio::sync::Mutex::new(None));

    let mut num_devices_open: usize = 0;
    loop {
        let start_num_devices_open = num_devices_open;

        // Read events that were added with `Watches::add` above.
        let mut buffer = [0; 1024];
        inotify
            .read_events_blocking(&mut buffer)?
            .filter_map(|event| match event.name.and_then(std::ffi::OsStr::to_str) {
                Some(name) if name.starts_with(video_file_prefix) => Some((name, event)),
                _ => None,
            })
            .for_each(|(name, event)| match event.mask {
                EventMask::OPEN => {
                    info!("Video device opened: {}", name);
                    num_devices_open = num_devices_open.saturating_add(1);
                }
                EventMask::CLOSE_WRITE | EventMask::CLOSE_NOWRITE => {
                    info!("Video device closed: {}", name);
                    num_devices_open = num_devices_open.saturating_sub(1);
                }
                _ => (),
            });

        // Since we're watching for events in `/dev`, we need to skip if the delta is 0
        // because it means there was no change in the number of video devices.
        if start_num_devices_open == num_devices_open {
            continue;
        }

        if num_devices_open == 0 {
            info!("Detected that a video device has been turned off.");

            let mut state = desired_state.lock().await;
            *state = Some(false);
        } else {
            info!("Detected that a video device has been turned on.");

            let mut state = desired_state.lock().await;
            *state = Some(true);
        };

        // Cancel any pending action
        if let Some(handle) = pending_action.take() {
            handle.abort();
        }

        // Clone variables for the async task
        let desired_state_clone = desired_state.clone();
        let context_clone = context.clone();
        let serial_number_clone = serial_number.map(|s| s.to_string());
        let device_path_clone = device_path.map(|s| s.to_string());
        let device_type_clone = device_type.map(|s| s.to_string());

        // Start a new delayed action
        pending_action = Some(tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

            let state = {
                let mut state = desired_state_clone.lock().await;
                state.take()
            };

            if let Some(state) = state {
                let mut context_lock = context_clone.lock().await;
                if state {
                    info!("Attempting to turn on Litra device(s)...");
                    let _ = turn_on_all_supported_devices_and_log(
                        &mut context_lock,
                        serial_number_clone.as_deref(),
                        device_path_clone.as_deref(),
                        device_type_clone.as_deref(),
                        require_device,
                    );
                } else {
                    info!("Attempting to turn off Litra device(s)...");
                    let _ = turn_off_all_supported_devices_and_log(
                        &mut context_lock,
                        serial_number_clone.as_deref(),
                        device_path_clone.as_deref(),
                        device_type_clone.as_deref(),
                        require_device,
                    );
                }
            }
        }));
    }
}

#[cfg(target_os = "macos")]
#[tokio::main]
async fn main() -> ExitCode {
    let args = Cli::parse();

    // Merge config file with CLI arguments (if config file is specified)
    let args = match merge_config_with_cli(args) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    let result = handle_autotoggle_command(
        args.serial_number.as_deref(),
        args.device_path.as_deref(),
        args.device_type.as_deref(),
        args.require_device,
        args.delay,
    )
    .await;

    if let Err(error) = result {
        error!("{error}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> ExitCode {
    let args = Cli::parse();

    // Merge config file with CLI arguments (if config file is specified)
    let args = match merge_config_with_cli(args) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    let result = handle_autotoggle_command(
        args.serial_number.as_deref(),
        args.device_path.as_deref(),
        args.device_type.as_deref(),
        args.require_device,
        args.video_device.as_deref(),
        args.delay,
    );

    if let Err(error) = result.await {
        error!("{}", error);
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Helper function to create a temporary YAML file with given content
    fn create_temp_config(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        file.write_all(content.as_bytes())
            .expect("Failed to write to temp file");
        file.flush().expect("Failed to flush temp file");
        file
    }

    #[test]
    fn test_load_valid_config_all_fields() {
        let config_content = r#"
serial_number: "ABC123"
delay: 2000
verbose: true
require_device: true
"#;
        let temp_file = create_temp_config(config_content);
        let config = load_config_file(&temp_file.path().to_path_buf()).unwrap();

        assert_eq!(config.serial_number, Some("ABC123".to_string()));
        assert_eq!(config.delay, Some(2000));
        assert_eq!(config.verbose, Some(true));
        assert_eq!(config.require_device, Some(true));
    }

    #[test]
    fn test_load_valid_config_device_type_glow() {
        let config_content = r#"
device_type: "glow"
"#;
        let temp_file = create_temp_config(config_content);
        let config = load_config_file(&temp_file.path().to_path_buf()).unwrap();

        assert_eq!(config.device_type, Some("glow".to_string()));
    }

    #[test]
    fn test_load_valid_config_device_type_beam() {
        let config_content = r#"
device_type: "beam"
"#;
        let temp_file = create_temp_config(config_content);
        let config = load_config_file(&temp_file.path().to_path_buf()).unwrap();

        assert_eq!(config.device_type, Some("beam".to_string()));
    }

    #[test]
    fn test_load_valid_config_device_type_beam_lx() {
        let config_content = r#"
device_type: "beam_lx"
"#;
        let temp_file = create_temp_config(config_content);
        let config = load_config_file(&temp_file.path().to_path_buf()).unwrap();

        assert_eq!(config.device_type, Some("beam_lx".to_string()));
    }

    #[test]
    fn test_load_valid_config_device_path() {
        let config_content = r#"
device_path: "/dev/hidraw0"
"#;
        let temp_file = create_temp_config(config_content);
        let config = load_config_file(&temp_file.path().to_path_buf()).unwrap();

        assert_eq!(config.device_path, Some("/dev/hidraw0".to_string()));
    }

    #[test]
    fn test_load_valid_config_empty() {
        let config_content = "";
        let temp_file = create_temp_config(config_content);
        let config = load_config_file(&temp_file.path().to_path_buf()).unwrap();

        assert_eq!(config.serial_number, None);
        assert_eq!(config.device_type, None);
        assert_eq!(config.device_path, None);
    }

    #[test]
    fn test_load_invalid_config_unknown_field() {
        let config_content = r#"
device_type: "glow"
unknown_field: "value"
"#;
        let temp_file = create_temp_config(config_content);
        let result = load_config_file(&temp_file.path().to_path_buf());

        assert!(result.is_err());
        match result {
            Err(CliError::ConfigFileError(msg)) => {
                assert!(msg.contains("unknown field"));
                assert!(msg.contains("unknown_field"));
            }
            _ => panic!("Expected ConfigFileError with unknown field message"),
        }
    }

    #[test]
    fn test_load_invalid_config_invalid_device_type() {
        let config_content = r#"
device_type: "invalid_type"
"#;
        let temp_file = create_temp_config(config_content);
        let result = load_config_file(&temp_file.path().to_path_buf());

        assert!(result.is_err());
        match result {
            Err(CliError::InvalidDeviceType(device_type)) => {
                assert_eq!(device_type, "invalid_type");
            }
            _ => panic!("Expected InvalidDeviceType error"),
        }
    }

    #[test]
    fn test_load_invalid_config_multiple_filters() {
        let config_content = r#"
serial_number: "ABC123"
device_type: "glow"
"#;
        let temp_file = create_temp_config(config_content);
        let result = load_config_file(&temp_file.path().to_path_buf());

        assert!(result.is_err());
        match result {
            Err(CliError::MultipleFiltersSpecified) => {}
            _ => panic!("Expected MultipleFiltersSpecified error"),
        }
    }

    #[test]
    fn test_load_invalid_config_invalid_yaml() {
        let config_content = r#"
device_type: [invalid
"#;
        let temp_file = create_temp_config(config_content);
        let result = load_config_file(&temp_file.path().to_path_buf());

        assert!(result.is_err());
        match result {
            Err(CliError::ConfigFileError(msg)) => {
                assert!(msg.contains("Failed to parse YAML"));
            }
            _ => panic!("Expected ConfigFileError with YAML parse error"),
        }
    }

    #[test]
    fn test_load_config_file_not_found() {
        let result = load_config_file(&PathBuf::from("/nonexistent/path/config.yaml"));

        assert!(result.is_err());
        match result {
            Err(CliError::ConfigFileError(msg)) => {
                assert!(msg.contains("Failed to read config file"));
            }
            _ => panic!("Expected ConfigFileError with read error"),
        }
    }

    #[test]
    fn test_validate_device_type_valid() {
        assert!(validate_device_type("glow").is_ok());
        assert!(validate_device_type("beam").is_ok());
        assert!(validate_device_type("beam_lx").is_ok());
    }

    #[test]
    fn test_validate_device_type_invalid() {
        assert!(validate_device_type("invalid").is_err());
        assert!(validate_device_type("").is_err());
        assert!(validate_device_type("GLOW").is_err()); // Case sensitive
    }

    #[test]
    fn test_validate_single_filter_none() {
        assert!(validate_single_filter(None, None, None).is_ok());
    }

    #[test]
    fn test_validate_single_filter_one() {
        assert!(validate_single_filter(Some("serial"), None, None).is_ok());
        assert!(validate_single_filter(None, Some("path"), None).is_ok());
        assert!(validate_single_filter(None, None, Some("glow")).is_ok());
    }

    #[test]
    fn test_validate_single_filter_multiple() {
        assert!(validate_single_filter(Some("serial"), Some("path"), None).is_err());
        assert!(validate_single_filter(Some("serial"), None, Some("glow")).is_err());
        assert!(validate_single_filter(None, Some("path"), Some("glow")).is_err());
        assert!(validate_single_filter(Some("serial"), Some("path"), Some("glow")).is_err());
    }

    #[test]
    fn test_config_deserialization_with_comments() {
        let config_content = r#"
# This is a comment
device_type: "glow"  # inline comment
delay: 2000
"#;
        let temp_file = create_temp_config(config_content);
        let config = load_config_file(&temp_file.path().to_path_buf()).unwrap();

        assert_eq!(config.device_type, Some("glow".to_string()));
        assert_eq!(config.delay, Some(2000));
    }
}
