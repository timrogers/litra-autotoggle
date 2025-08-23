use clap::{builder::TypedValueParser, Parser};
#[cfg(target_os = "linux")]
use inotify::{EventMask, Inotify, WatchMask};
use litra::{Device, DeviceError, DeviceHandle, DeviceResult, DeviceType, Litra};
#[cfg(target_os = "macos")]
use log::debug;
use log::{error, info};
use std::fmt;
use std::process::ExitCode;
#[cfg(target_os = "macos")]
use std::process::Stdio;
use std::str::FromStr;
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tokio::io::{AsyncBufReadExt, BufReader};
#[cfg(target_os = "macos")]
use tokio::process::Command;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
struct DeviceTypeValueParser;

impl TypedValueParser for DeviceTypeValueParser {
    type Value = DeviceType;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let value_str = value.to_string_lossy();
        DeviceType::from_str(&value_str).map_err(|_| {
            let mut err = clap::Error::new(clap::error::ErrorKind::InvalidValue);
            if let Some(arg) = arg {
                err.insert(
                    clap::error::ContextKind::InvalidArg,
                    clap::error::ContextValue::String(arg.to_string()),
                );
            }
            err.insert(
                clap::error::ContextKind::Custom,
                clap::error::ContextValue::String(format!("Invalid device type: {value_str}")),
            );
            err
        })
    }
}

/// Automatically turn your Logitech Litra device on when your webcam turns on, and off when your webcam turns off (macOS and Linux only).
#[derive(Debug, Parser)]
#[clap(name = "litra-autotoggle", version)]
struct Cli {
    #[clap(long, short, help = "The serial number of the Logitech Litra device")]
    serial_number: Option<String>,

    #[clap(long, short, help = "The device path of the Logitech Litra device")]
    device_path: Option<String>,

    #[clap(
        long,
        help = "The type of the Logitech Litra device (`glow`, `beam` or `beam_lx`)",
        value_parser = DeviceTypeValueParser
    )]
    device_type: Option<DeviceType>,

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

#[derive(Debug)]
enum CliError {
    DeviceError(DeviceError),
    IoError(std::io::Error),
    DeviceNotFound,
    MultipleFiltersSpecified,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::DeviceError(error) => error.fmt(f),
            CliError::IoError(error) => write!(f, "Input/output error: {error}"),
            CliError::DeviceNotFound => write!(
                f,
                "Device not found"
            ),
            CliError::MultipleFiltersSpecified => write!(f, "Only one filter (--serial-number, --device-path, or --device-type) can be specified at a time"),
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
    device_type: Option<&DeviceType>,
) -> Result<(), CliError> {
    let filter_count = serial_number.is_some() as usize
        + device_path.is_some() as usize
        + device_type.is_some() as usize;

    if filter_count > 1 {
        Err(CliError::MultipleFiltersSpecified)
    } else {
        Ok(())
    }
}

/// Get all devices matching the given filters
fn get_all_supported_devices(
    context: &mut Litra,
    serial_number: Option<&str>,
    device_path: Option<&str>,
    device_type: Option<&DeviceType>,
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
    if let Some(serial) = serial_number {
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
        Ok(handles)
    } else {
        // No serial filter, include all devices that matched the other filters
        Ok(potential_devices
            .into_iter()
            .filter_map(|dev| dev.open(context).ok())
            .collect())
    }
}

fn check_device_filters<'a>(
    _context: &'a Litra,
    _serial_number: Option<&'a str>,
    device_path: Option<&'a str>,
    device_type: Option<&'a DeviceType>,
) -> impl Fn(&Device) -> bool + 'a {
    move |device| {
        // Check device path if specified
        if let Some(path) = device_path {
            return device.device_path() == path;
        }

        // Check device type if specified
        if let Some(expected_type) = device_type {
            if device.device_type() != *expected_type {
                return false;
            }
        }

        // If a serial number is specified, we'll filter by it after opening the device
        // since serial numbers are only accessible after opening
        true
    }
}

/// Apply a command to device(s)
fn with_device<F>(
    context: &mut Litra,
    serial_number: Option<&str>,
    device_path: Option<&str>,
    device_type: Option<&DeviceType>,
    require_device: bool,
    callback: F,
) -> CliResult
where
    F: Fn(&DeviceHandle) -> DeviceResult<()>,
{
    // Default to all matching devices or explicit filter
    let use_all = serial_number.is_none() && device_path.is_none() && device_type.is_none();

    if use_all {
        // Get all devices
        let devices = get_all_supported_devices(context, None, None, None)?;
        if devices.is_empty() && require_device {
            return Err(CliError::DeviceNotFound);
        }

        for device_handle in devices {
            // Ignore errors for individual devices when targeting all
            let _ = callback(&device_handle);
        }
        Ok(())
    } else {
        // Filtering by one of the options
        let devices = get_all_supported_devices(context, serial_number, device_path, device_type)?;
        if devices.is_empty() && require_device {
            return Err(CliError::DeviceNotFound);
        }

        // Apply to all matched devices
        for device_handle in devices {
            // Ignore errors for individual devices
            let _ = callback(&device_handle);
        }
        Ok(())
    }
}

fn turn_on_supported_devices_and_log(
    context: &mut Litra,
    serial_number: Option<&str>,
    device_path: Option<&str>,
    device_type: Option<&DeviceType>,
    require_device: bool,
) -> Result<(), CliError> {
    with_device(
        context,
        serial_number,
        device_path,
        device_type,
        require_device,
        |device_handle| {
            info!("Turning on {} device", device_handle.device_type());
            device_handle.set_on(true)
        },
    )?;

    Ok(())
}

fn turn_off_supported_devices_and_log(
    context: &mut Litra,
    serial_number: Option<&str>,
    device_path: Option<&str>,
    device_type: Option<&DeviceType>,
    require_device: bool,
) -> Result<(), CliError> {
    with_device(
        context,
        serial_number,
        device_path,
        device_type,
        require_device,
        |device_handle| {
            info!("Turning off {} device", device_handle.device_type());
            device_handle.set_on(false)
        },
    )?;

    Ok(())
}

#[cfg(target_os = "macos")]
async fn handle_autotoggle_command(
    serial_number: Option<&str>,
    device_path: Option<&str>,
    device_type: Option<DeviceType>,
    require_device: bool,
    delay: u64,
) -> CliResult {
    // Wrap context in Arc<Mutex<>> to enable sharing across tasks
    let context = Arc::new(Mutex::new(Litra::new()?));

    // Use context inside an async block with locking
    {
        let context_clone = context.clone();
        let mut context_lock = context_clone.lock().await;
        let devices = get_all_supported_devices(
            &mut context_lock,
            serial_number,
            device_path,
            device_type.as_ref(),
        )?;

        if devices.is_empty() && require_device {
            return Err(CliError::DeviceNotFound);
        }

        for device_handle in &devices {
            info!("Found {} device", device_handle.device_type());
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
    let (exit_tx, mut exit_rx) = tokio::sync::mpsc::channel::<()>(1);

    loop {
        tokio::select! {
            log_line_result = reader.next_line() => {
                let log_line = log_line_result
                    .expect("Failed to read log line from `log` process when listening for video device events");

                if let Some(log_line) = log_line {
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
                        let device_type_clone = device_type;
                        let exit_tx_clone = exit_tx.clone();

                        // Start a new delayed action
                        pending_action = Some(tokio::spawn(async move {
                            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

                            let state = {
                                let mut state = desired_state_clone.lock().await;
                                state.take()
                            };

                            if let Some(state) = state {
                                let mut context_lock = context_clone.lock().await;
                                let result = if state {
                                    info!("Attempting to turn on Litra device(s)...");
                                    turn_on_supported_devices_and_log(
                                        &mut context_lock,
                                        serial_number_clone.as_deref(),
                                        device_path_clone.as_deref(),
                                        device_type_clone.as_ref(),
                                        require_device,
                                    )
                                } else {
                                    info!("Attempting to turn off Litra device(s)...");
                                    turn_off_supported_devices_and_log(
                                        &mut context_lock,
                                        serial_number_clone.as_deref(),
                                        device_path_clone.as_deref(),
                                        device_type_clone.as_ref(),
                                        require_device,
                                    )
                                };

                                if let Err(_error) = result {
                                    let _ = exit_tx_clone.send(()).await;
                                    return;
                                }
                            }
                        }));
                    }
                } else {
                    // End of stream
                    break;
                }
            }
            _ = exit_rx.recv() => {
                return Err(CliError::DeviceNotFound);
            }
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
    device_type: Option<DeviceType>,
    require_device: bool,
    video_device: Option<&str>,
    delay: u64,
) -> CliResult {
    // Wrap context in Arc<Mutex<>> to enable sharing across tasks
    let context = Arc::new(Mutex::new(Litra::new()?));

    // Use context inside an async block with locking
    {
        let mut context_lock = context.lock().await;
        let devices = get_all_supported_devices(
            &mut context_lock,
            serial_number,
            device_path,
            device_type.as_ref(),
        )?;

        if devices.is_empty() && require_device {
            return Err(CliError::DeviceNotFound);
        }

        if let Some(device_handle) = devices.first() {
            info!("Found {} device", device_handle.device_type());
        }
    }

    // Path to watch for video device events
    let watch_path = video_device.unwrap_or("/dev");

    // Extract video device name from path, or use "video" as default
    let video_file_prefix = video_device
        .and_then(|p| p.split('/').last())
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
    let (exit_tx, mut exit_rx) = tokio::sync::mpsc::channel::<()>(1);

    let mut num_devices_open: usize = 0;
    loop {
        let mut buffer = [0; 1024];

        tokio::select! {
            events_result = tokio::task::spawn_blocking({
                let mut inotify_clone = inotify;
                move || {
                    inotify_clone.read_events_blocking(&mut buffer)
                }
            }) => {
                inotify = events_result.unwrap()?;
                let events: Vec<_> = inotify
                    .filter_map(|event| match event.name.and_then(std::ffi::OsStr::to_str) {
                        Some(name) if name.starts_with(video_file_prefix) => Some((name.to_string(), event)),
                        _ => None,
                    })
                    .collect();

                let start_num_devices_open = num_devices_open;

                for (name, event) in events {
                    match event.mask {
                        EventMask::OPEN => {
                            info!("Video device opened: {}", name);
                            num_devices_open = num_devices_open.saturating_add(1);
                        }
                        EventMask::CLOSE_WRITE | EventMask::CLOSE_NOWRITE => {
                            info!("Video device closed: {}", name);
                            num_devices_open = num_devices_open.saturating_sub(1);
                        }
                        _ => (),
                    }
                }

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
                let device_path_clone = device_path.map(|p| p.to_string());
                let device_type_clone = device_type.clone();
                let exit_tx_clone = exit_tx.clone();

                // Start a new delayed action
                pending_action = Some(tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

                    let state = {
                        let mut state = desired_state_clone.lock().await;
                        state.take()
                    };

                    if let Some(state) = state {
                        let mut context_lock = context_clone.lock().await;
                        let result = if state {
                            info!("Attempting to turn on Litra device...");
                            turn_on_supported_devices_and_log(
                                &mut context_lock,
                                serial_number_clone.as_deref(),
                                device_path_clone.as_deref(),
                                device_type_clone.as_ref(),
                                require_device,
                            )
                        } else {
                            info!("Attempting to turn off Litra device...");
                            turn_off_supported_devices_and_log(
                                &mut context_lock,
                                serial_number_clone.as_deref(),
                                device_path_clone.as_deref(),
                                device_type_clone.as_ref(),
                                require_device,
                            )
                        };

                        if let Err(error) = result {
                            let _ = exit_tx_clone.send(()).await;
                            return;
                        }
                    }
                }));
            }
            _ = exit_rx.recv() => {
                return Err(CliError::DeviceNotFound);
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[tokio::main]
async fn main() -> ExitCode {
    let args = Cli::parse();

    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    let result = handle_autotoggle_command(
        args.serial_number.as_deref(),
        args.device_path.as_deref(),
        args.device_type,
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

    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    let result = handle_autotoggle_command(
        args.serial_number.as_deref(),
        args.device_path.as_deref(),
        args.device_type,
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
