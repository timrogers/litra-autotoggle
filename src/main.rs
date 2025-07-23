use clap::Parser;
#[cfg(target_os = "linux")]
use inotify::{EventMask, Inotify, WatchMask};
use litra::{Device, DeviceError, DeviceHandle, Litra};
#[cfg(target_os = "macos")]
use log::debug;
use log::{error, info, warn};
use std::fmt;
use std::process::ExitCode;
#[cfg(target_os = "macos")]
use std::process::Stdio;
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tokio::io::{AsyncBufReadExt, BufReader};
#[cfg(target_os = "macos")]
use tokio::process::Command;
use tokio::sync::Mutex;

/// Automatically turn your Logitech Litra device on when your webcam turns on, and off when your webcam turns off (macOS and Linux only).
#[derive(Debug, Parser)]
#[clap(name = "litra-autotoggle", version)]
struct Cli {
    #[clap(long, short, help = "The serial number of the Logitech Litra device")]
    serial_number: Option<String>,

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

fn check_serial_number_if_some(serial_number: Option<&str>) -> impl Fn(&Device) -> bool + '_ {
    move |device| {
        serial_number.as_ref().is_none_or(|expected| {
            device
                .device_info()
                .serial_number()
                .is_some_and(|actual| &actual == expected)
        })
    }
}

#[derive(Debug)]
enum CliError {
    DeviceError(DeviceError),
    IoError(std::io::Error),
    NoDevicesFound,
    DeviceNotFound(String),
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

fn get_first_supported_device(
    context: &mut Litra,
    serial_number: Option<&str>,
    require_device: bool,
) -> Result<Option<DeviceHandle>, CliError> {
    {
        context.refresh_connected_devices()?;
    }

    match context
        .get_connected_devices()
        .find(check_serial_number_if_some(serial_number))
    {
        Some(device_handle) => device_handle
            .open(context)
            .map(Some)
            .map_err(CliError::DeviceError),
        None => {
            if require_device {
                if let Some(serial_number) = serial_number {
                    Err(CliError::DeviceNotFound(serial_number.to_string()))
                } else {
                    Err(CliError::NoDevicesFound)
                }
            } else {
                Ok(None)
            }
        }
    }
}

fn turn_on_first_supported_device_and_log(
    context: &mut Litra,
    serial_number: Option<&str>,
    require_device: bool,
) -> Result<(), CliError> {
    if let Some(device_handle) = get_first_supported_device(context, serial_number, require_device)?
    {
        info!(
            "Turning on {} device (serial number: {})",
            device_handle.device_type(),
            get_serial_number_with_fallback(&device_handle)
        );

        device_handle.set_on(true)?;
    } else {
        print_device_not_found_log(serial_number);
    }

    Ok(())
}

fn turn_off_first_supported_device_and_log(
    context: &mut Litra,
    serial_number: Option<&str>,
    require_device: bool,
) -> Result<(), CliError> {
    if let Some(device_handle) = get_first_supported_device(context, serial_number, require_device)?
    {
        info!(
            "Turning off {} device (serial number: {})",
            device_handle.device_type(),
            get_serial_number_with_fallback(&device_handle)
        );

        device_handle.set_on(false)?;
    } else {
        print_device_not_found_log(serial_number);
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
    require_device: bool,
    delay: u64,
) -> CliResult {
    // Wrap context in Arc<Mutex<>> to enable sharing across tasks
    let context = Arc::new(Mutex::new(Litra::new()?));

    // Use context inside an async block with locking
    {
        let mut context_lock = context.lock().await;
        if let Some(device_handle) =
            get_first_supported_device(&mut context_lock, serial_number, require_device)?
        {
            info!(
                "Found {} device (serial number: {})",
                device_handle.device_type(),
                get_serial_number_with_fallback(&device_handle)
            );
        } else {
            print_device_not_found_log(serial_number);
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
                        info!("Attempting to turn on Litra device...");
                        let _ = turn_on_first_supported_device_and_log(
                            &mut context_lock,
                            serial_number_clone.as_deref(),
                            require_device,
                        );
                    } else {
                        info!("Attempting to turn off Litra device...");
                        let _ = turn_off_first_supported_device_and_log(
                            &mut context_lock,
                            serial_number_clone.as_deref(),
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
    require_device: bool,
    video_device: Option<&str>,
    delay: u64,
) -> CliResult {
    // Wrap context in Arc<Mutex<>> to enable sharing across tasks
    let context = Arc::new(Mutex::new(Litra::new()?));

    // Use context inside an async block with locking
    {
        let mut context_lock = context.lock().await;
        if let Some(device_handle) =
            get_first_supported_device(&mut context_lock, serial_number, require_device)?
        {
            info!(
                "Found {} device (serial number: {})",
                device_handle.device_type(),
                get_serial_number_with_fallback(&device_handle)
            );
        } else {
            print_device_not_found_log(serial_number);
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
                    info!("Attempting to turn on Litra device...");
                    let _ = turn_on_first_supported_device_and_log(
                        &mut context_lock,
                        serial_number_clone.as_deref(),
                        require_device,
                    );
                } else {
                    info!("Attempting to turn off Litra device...");
                    let _ = turn_off_first_supported_device_and_log(
                        &mut context_lock,
                        serial_number_clone.as_deref(),
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

    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    let result = handle_autotoggle_command(
        args.serial_number.as_deref(),
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
