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
#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

/// Configuration structure for YAML file deserialization.
/// Field names use underscores to match YAML convention (e.g. serial_number).
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct AppConfig {
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

    #[serde(skip_serializing_if = "Option::is_none")]
    back: Option<bool>,
}

/// Automatically turn your Logitech Litra device on when your webcam turns on, and off when your webcam turns off.
#[derive(Debug, Parser)]
#[clap(name = "litra-autotoggle", version, after_long_help = "This CLI automatically checks for updates once per day. To disable update checks, set the LITRA_AUTOTOGGLE_DISABLE_UPDATE_CHECK environment variable to any value.")]
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

    #[clap(
        long,
        short = 'b',
        action,
        help = "Toggle the back light on Litra Beam LX devices. When enabled, the back light will be turned on/off together with the front light."
    )]
    back: bool,
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
fn load_config_file(config_path: &PathBuf) -> Result<AppConfig, CliError> {
    // Read the file
    let contents = fs::read_to_string(config_path)
        .map_err(|e| CliError::ConfigFileError(format!("Failed to read config file: {}", e)))?;

    // Parse YAML
    let config: AppConfig = serde_yaml::from_str(&contents)
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
        if !cli.back {
            cli.back = config.back.unwrap_or(false);
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

/// Helper function to toggle the back light on Litra Beam LX devices.
/// Only toggles if the device is a Beam LX and the `back` flag is enabled.
fn toggle_back_light_if_applicable(device_handle: &DeviceHandle, on: bool, back: bool) {
    if back && device_handle.device_type() == litra::DeviceType::LitraBeamLX {
        let action = if on { "on" } else { "off" };
        info!(
            "Turning {} back light for {} device (serial number: {})",
            action,
            device_handle.device_type(),
            get_serial_number_with_fallback(device_handle)
        );
        if let Err(e) = device_handle.set_back_on(on) {
            warn!(
                "Failed to turn {} back light for {} device (serial number: {}): {}",
                action,
                device_handle.device_type(),
                get_serial_number_with_fallback(device_handle),
                e
            );
        }
    }
}

fn turn_on_all_supported_devices_and_log(
    context: &mut Litra,
    serial_number: Option<&str>,
    device_path: Option<&str>,
    device_type: Option<&str>,
    require_device: bool,
    back: bool,
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

            toggle_back_light_if_applicable(&device_handle, true, back);
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
    back: bool,
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

            toggle_back_light_if_applicable(&device_handle, false, back);
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
    back: bool,
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
                            back,
                        );
                    } else {
                        info!("Attempting to turn off Litra device(s)...");
                        let _ = turn_off_all_supported_devices_and_log(
                            &mut context_lock,
                            serial_number_clone.as_deref(),
                            device_path_clone.as_deref(),
                            device_type_clone.as_deref(),
                            require_device,
                            back,
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
    back: bool,
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
                        back,
                    );
                } else {
                    info!("Attempting to turn off Litra device(s)...");
                    let _ = turn_off_all_supported_devices_and_log(
                        &mut context_lock,
                        serial_number_clone.as_deref(),
                        device_path_clone.as_deref(),
                        device_type_clone.as_deref(),
                        require_device,
                        back,
                    );
                }
            }
        }));
    }
}

#[cfg(target_os = "windows")]
const WEBCAM_REGISTRY_PATH: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\webcam";

#[cfg(target_os = "windows")]
const NONPACKAGED_APPS_KEY: &str = "NonPackaged";

#[cfg(target_os = "windows")]
const REGISTRY_POLL_INTERVAL_MS: u64 = 500;

#[cfg(target_os = "windows")]
async fn handle_autotoggle_command(
    serial_number: Option<&str>,
    device_path: Option<&str>,
    device_type: Option<&str>,
    require_device: bool,
    delay: u64,
    back: bool,
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

    info!("Monitoring Windows registry for webcam usage...");

    // Add variables for throttling
    let mut pending_action: Option<tokio::task::JoinHandle<()>> = None;
    let desired_state = Arc::new(tokio::sync::Mutex::new(None));

    // Track previous camera state
    let mut previous_camera_in_use = false;

    // Registry path for webcam access
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let webcam_path = WEBCAM_REGISTRY_PATH;

    loop {
        // Poll the registry for camera usage
        let camera_in_use = match hkcu.open_subkey(webcam_path) {
            Ok(webcam_key) => {
                let mut any_camera_active = false;

                // Iterate through all subkeys (apps that have accessed the camera)
                for subkey_name in webcam_key.enum_keys().filter_map(|k| k.ok()) {
                    if let Ok(app_key) = webcam_key.open_subkey(&subkey_name) {
                        // Skip the "NonPackaged" key itself, only look at its children
                        if subkey_name == NONPACKAGED_APPS_KEY {
                            for nonpackaged_subkey in app_key.enum_keys().filter_map(|k| k.ok()) {
                                if let Ok(nonpackaged_app_key) =
                                    app_key.open_subkey(&nonpackaged_subkey)
                                {
                                    if is_camera_active(&nonpackaged_app_key) {
                                        any_camera_active = true;
                                        break;
                                    }
                                }
                            }
                            if any_camera_active {
                                break;
                            }
                        } else {
                            // Regular packaged apps
                            if is_camera_active(&app_key) {
                                any_camera_active = true;
                                break;
                            }
                        }
                    }

                    if any_camera_active {
                        break;
                    }
                }

                any_camera_active
            }
            Err(e) => {
                warn!(
                    "Failed to access webcam registry key: {}. Assuming camera is not in use.",
                    e
                );
                false
            }
        };

        // Only trigger action if state has changed
        if camera_in_use != previous_camera_in_use {
            previous_camera_in_use = camera_in_use;

            if camera_in_use {
                info!("Detected that a video device has been turned on.");
                let mut state = desired_state.lock().await;
                *state = Some(true);
            } else {
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
                            back,
                        );
                    } else {
                        info!("Attempting to turn off Litra device(s)...");
                        let _ = turn_off_all_supported_devices_and_log(
                            &mut context_lock,
                            serial_number_clone.as_deref(),
                            device_path_clone.as_deref(),
                            device_type_clone.as_deref(),
                            require_device,
                            back,
                        );
                    }
                }
            }));
        }

        // Poll every REGISTRY_POLL_INTERVAL_MS to check for camera state changes
        tokio::time::sleep(tokio::time::Duration::from_millis(
            REGISTRY_POLL_INTERVAL_MS,
        ))
        .await;
    }
}

#[cfg(target_os = "windows")]
fn is_camera_active(app_key: &RegKey) -> bool {
    // Read LastUsedTimeStart and LastUsedTimeStop
    let start: Result<u64, _> = app_key.get_value("LastUsedTimeStart");
    let stop: Result<u64, _> = app_key.get_value("LastUsedTimeStop");

    match (start, stop) {
        (Ok(start_time), Ok(stop_time)) => {
            // Camera is active if start time is greater than stop time
            start_time > stop_time
        }
        (Ok(_), Err(_)) => {
            // If we have a start time but no stop time, camera might be in use
            true
        }
        _ => false,
    }
}

/// The current version of the CLI, extracted from Cargo.toml
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// GitHub API URL for fetching releases (list endpoint)
const GITHUB_API_URL: &str =
    "https://api.github.com/repos/timrogers/litra-autotoggle/releases";

/// Timeout for update check requests in seconds
const UPDATE_CHECK_TIMEOUT_SECS: u64 = 2;

/// Response structure for GitHub releases API
#[derive(serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    published_at: String,
}

/// Configuration file name for storing update check state
const UPDATE_CONFIG_FILE_NAME: &str = ".litra-autotoggle.toml";

/// Number of seconds in a day (24 hours)
const SECONDS_PER_DAY: u64 = 86400;

/// Configuration structure for the TOML update check state file
#[derive(serde::Deserialize, serde::Serialize, Default)]
struct UpdateConfig {
    #[serde(default)]
    update_check: UpdateCheckConfig,
}

/// Update check configuration
#[derive(serde::Deserialize, serde::Serialize, Default)]
struct UpdateCheckConfig {
    /// Unix timestamp of the last update check
    last_check_timestamp: Option<u64>,
}

/// Returns the path to the update config file in the user's home directory
fn get_update_config_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(UPDATE_CONFIG_FILE_NAME))
}

/// Reads the update config file, returning a default config if the file doesn't exist or can't be read
fn read_update_config() -> UpdateConfig {
    let Some(config_path) = get_update_config_path() else {
        return UpdateConfig::default();
    };

    match std::fs::read_to_string(&config_path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
        Err(_) => UpdateConfig::default(),
    }
}

/// Writes the update config to the config file, silently ignoring errors
fn write_update_config(config: &UpdateConfig) {
    let Some(config_path) = get_update_config_path() else {
        return;
    };

    if let Ok(contents) = toml::to_string_pretty(config) {
        let _ = std::fs::write(&config_path, contents);
    }
}

/// Returns the current Unix timestamp in seconds
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Checks if enough time has passed since the last update check (at least one day)
fn should_check_for_updates(config: &UpdateConfig) -> bool {
    let Some(last_check) = config.update_check.last_check_timestamp else {
        return true; // Never checked before
    };

    let now = current_timestamp();
    now.saturating_sub(last_check) >= SECONDS_PER_DAY
}

/// Checks if a release is old enough to be considered for update notifications (at least 72 hours)
fn is_release_old_enough(published_at: &str) -> bool {
    use chrono::{DateTime, Duration, Utc};

    let Ok(release_time) = DateTime::parse_from_rfc3339(published_at) else {
        return false;
    };

    let cutoff = Utc::now() - Duration::hours(72);
    release_time < cutoff
}

/// Environment variable to disable update checks
const DISABLE_UPDATE_CHECK_ENV: &str = "LITRA_AUTOTOGGLE_DISABLE_UPDATE_CHECK";

/// Compares two semantic version strings to determine if `latest` is newer than `current`.
/// Returns true if `latest` is a newer version.
fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_version = |v: &str| -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() >= 3 {
            Some((
                parts[0].parse().ok()?,
                parts[1].parse().ok()?,
                parts[2].parse().ok()?,
            ))
        } else if parts.len() == 2 {
            Some((parts[0].parse().ok()?, parts[1].parse().ok()?, 0))
        } else if parts.len() == 1 {
            Some((parts[0].parse().ok()?, 0, 0))
        } else {
            None
        }
    };

    match (parse_version(latest), parse_version(current)) {
        (Some((l_major, l_minor, l_patch)), Some((c_major, c_minor, c_patch))) => {
            (l_major, l_minor, l_patch) > (c_major, c_minor, c_patch)
        }
        _ => false,
    }
}

/// Checks for updates by fetching releases from GitHub.
/// Returns the latest version tag if a newer version is available, None otherwise.
/// This function will timeout after 2 seconds but will not disrupt normal operation.
/// Set the LITRA_AUTOTOGGLE_DISABLE_UPDATE_CHECK environment variable to disable this check.
/// The check is performed at most once per day, with the last check time stored in ~/.litra-autotoggle.toml.
/// Only releases that are at least 72 hours old are considered.
fn check_for_updates() -> Option<String> {
    if std::env::var(DISABLE_UPDATE_CHECK_ENV).is_ok() {
        return None;
    }

    let mut config = read_update_config();
    if !should_check_for_updates(&config) {
        return None;
    }

    config.update_check.last_check_timestamp = Some(current_timestamp());
    write_update_config(&config);

    let timeout = std::time::Duration::from_secs(UPDATE_CHECK_TIMEOUT_SECS);

    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .build(),
    );

    let mut response = match agent
        .get(GITHUB_API_URL)
        .header("User-Agent", format!("litra-autotoggle/{}", CURRENT_VERSION))
        .header("Accept", "application/vnd.github.v3+json")
        .call()
    {
        Ok(response) => response,
        Err(e) => {
            if let ureq::Error::Timeout(_) = e {
                eprintln!(
                    "Warning: Update check timed out after {} seconds",
                    UPDATE_CHECK_TIMEOUT_SECS
                );
            }
            return None;
        }
    };

    let releases: Vec<GitHubRelease> = match response.body_mut().read_json() {
        Ok(releases) => releases,
        Err(_) => return None,
    };

    let mut best_version: Option<String> = None;

    for release in releases {
        if !is_release_old_enough(&release.published_at) {
            continue;
        }

        let release_version = release.tag_name.trim_start_matches('v');

        if is_newer_version(release_version, CURRENT_VERSION) {
            match &best_version {
                None => best_version = Some(release.tag_name),
                Some(current_best) => {
                    let current_best_version = current_best.trim_start_matches('v');
                    if is_newer_version(release_version, current_best_version) {
                        best_version = Some(release.tag_name);
                    }
                }
            }
        }
    }

    best_version
}

/// Generates the update notification message for the given version
fn format_update_message(latest_version: &str) -> String {
    format!(
        "A new version of litra-autotoggle is available: {} (current: v{}). Download the latest release at https://github.com/timrogers/litra-autotoggle/releases/tag/{}",
        latest_version, CURRENT_VERSION, latest_version
    )
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

    if let Some(latest_version) = check_for_updates() {
        warn!("{}", format_update_message(&latest_version));
    }

    let result = handle_autotoggle_command(
        args.serial_number.as_deref(),
        args.device_path.as_deref(),
        args.device_type.as_deref(),
        args.require_device,
        args.delay,
        args.back,
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

    if let Some(latest_version) = check_for_updates() {
        warn!("{}", format_update_message(&latest_version));
    }

    let result = handle_autotoggle_command(
        args.serial_number.as_deref(),
        args.device_path.as_deref(),
        args.device_type.as_deref(),
        args.require_device,
        args.video_device.as_deref(),
        args.delay,
        args.back,
    )
    .await;

    if let Err(error) = result {
        error!("{}", error);
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(target_os = "windows")]
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

    if let Some(latest_version) = check_for_updates() {
        warn!("{}", format_update_message(&latest_version));
    }

    let result = handle_autotoggle_command(
        args.serial_number.as_deref(),
        args.device_path.as_deref(),
        args.device_type.as_deref(),
        args.require_device,
        args.delay,
        args.back,
    )
    .await;

    if let Err(error) = result {
        error!("{error}");
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

    #[test]
    fn test_is_newer_version_major() {
        assert!(is_newer_version("4.0.0", "3.2.0"));
        assert!(is_newer_version("2.0.0", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "2.0.0"));
        assert!(!is_newer_version("3.0.0", "4.0.0"));
    }

    #[test]
    fn test_is_newer_version_minor() {
        assert!(is_newer_version("1.4.0", "1.3.0"));
        assert!(is_newer_version("1.2.0", "1.1.0"));
        assert!(!is_newer_version("1.1.0", "1.2.0"));
        assert!(!is_newer_version("1.3.0", "1.4.0"));
    }

    #[test]
    fn test_is_newer_version_patch() {
        assert!(is_newer_version("1.3.1", "1.3.0"));
        assert!(is_newer_version("1.0.5", "1.0.4"));
        assert!(!is_newer_version("1.0.4", "1.0.5"));
        assert!(!is_newer_version("1.3.0", "1.3.1"));
    }

    #[test]
    fn test_is_newer_version_same_version() {
        assert!(!is_newer_version("1.3.0", "1.3.0"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
    }

    #[test]
    fn test_is_newer_version_edge_cases() {
        assert!(is_newer_version("2.0", "1.9"));
        assert!(!is_newer_version("1.9", "2.0"));
        assert!(is_newer_version("2", "1"));
        assert!(!is_newer_version("1", "2"));
        assert!(!is_newer_version("invalid", "1.3.0"));
        assert!(!is_newer_version("1.3.0", "invalid"));
        assert!(!is_newer_version("", "1.3.0"));
    }

    #[test]
    fn test_should_check_for_updates_never_checked() {
        let config = UpdateConfig::default();
        assert!(should_check_for_updates(&config));
    }

    #[test]
    fn test_should_check_for_updates_checked_recently() {
        let mut config = UpdateConfig::default();
        config.update_check.last_check_timestamp = Some(current_timestamp());
        assert!(!should_check_for_updates(&config));
    }

    #[test]
    fn test_should_check_for_updates_checked_long_ago() {
        let mut config = UpdateConfig::default();
        config.update_check.last_check_timestamp =
            Some(current_timestamp() - SECONDS_PER_DAY - 1);
        assert!(should_check_for_updates(&config));
    }

    #[test]
    fn test_should_check_for_updates_exactly_one_day() {
        let mut config = UpdateConfig::default();
        config.update_check.last_check_timestamp = Some(current_timestamp() - SECONDS_PER_DAY);
        assert!(should_check_for_updates(&config));
    }

    #[test]
    fn test_is_release_old_enough() {
        assert!(is_release_old_enough("2020-01-01T00:00:00Z"));
        assert!(!is_release_old_enough("2099-01-01T00:00:00Z"));
        assert!(!is_release_old_enough("invalid"));
    }

    #[test]
    fn test_format_update_message() {
        let message = format_update_message("v1.4.0");
        assert!(message.contains("v1.4.0"));
        assert!(message.contains(CURRENT_VERSION));
        assert!(
            message.contains("https://github.com/timrogers/litra-autotoggle/releases/tag/v1.4.0")
        );
    }

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
back: true
"#;
        let temp_file = create_temp_config(config_content);
        let config = load_config_file(&temp_file.path().to_path_buf()).unwrap();

        assert_eq!(config.serial_number, Some("ABC123".to_string()));
        assert_eq!(config.delay, Some(2000));
        assert_eq!(config.verbose, Some(true));
        assert_eq!(config.require_device, Some(true));
        assert_eq!(config.back, Some(true));
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

    #[test]
    fn test_load_valid_config_back_option() {
        let config_content = r#"
back: true
"#;
        let temp_file = create_temp_config(config_content);
        let config = load_config_file(&temp_file.path().to_path_buf()).unwrap();

        assert_eq!(config.back, Some(true));
    }

    #[test]
    fn test_load_valid_config_back_option_false() {
        let config_content = r#"
back: false
"#;
        let temp_file = create_temp_config(config_content);
        let config = load_config_file(&temp_file.path().to_path_buf()).unwrap();

        assert_eq!(config.back, Some(false));
    }
}
