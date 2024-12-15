use clap::Parser;
#[cfg(target_os = "linux")]
use inotify::{EventMask, Inotify, WatchMask};
use litra::{Device, DeviceError, DeviceHandle, Litra};
use std::fmt;
use std::process::ExitCode;
#[cfg(target_os = "macos")]
use std::process::Stdio;
#[cfg(target_os = "macos")]
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tokio::io::{AsyncBufReadExt, BufReader};
#[cfg(target_os = "macos")]
use tokio::process::Command;
#[cfg(target_os = "macos")]
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

    #[cfg(target_os = "macos")]
    #[clap(
        long,
        short,
        default_value = "1500",
        help = "The delay in milliseconds between detecting a webcam event and toggling the Litra (macOS only). When your webcam turns on or off, multiple events may be generated in quick succession. Setting a delay allows the program to wait for all events before taking action, avoiding flickering."
    )]
    delay: u64,

    #[clap(long, short, action, help = "Output detailed log messages")]
    verbose: bool,
}

#[cfg(target_os = "linux")]
fn get_video_device_paths() -> std::io::Result<Vec<std::path::PathBuf>> {
    Ok(std::fs::read_dir("/dev")?
        .filter_map(|entry| entry.ok())
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .filter(|name| name.starts_with("video"))
                .map(|_| e.path())
        })
        .collect())
}

fn check_serial_number_if_some(serial_number: Option<&str>) -> impl Fn(&Device) -> bool + '_ {
    move |device| {
        serial_number.as_ref().map_or(true, |expected| {
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
            CliError::IoError(error) => write!(f, "Input/output error: {}", error),
            CliError::NoDevicesFound => write!(f, "No Litra devices found"),
            CliError::DeviceNotFound(serial_number) => write!(
                f,
                "Litra device with serial number {} not found",
                serial_number
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
        println!(
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
        println!(
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
        println!(
            "Litra device with serial number {} not found",
            serial_number.unwrap()
        );
    } else {
        println!("No Litra devices found");
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
    verbose: bool,
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
            println!(
                "Found {} device (serial number: {})",
                device_handle.device_type(),
                get_serial_number_with_fallback(&device_handle)
            );
        } else {
            print_device_not_found_log(serial_number);
        }
    }

    println!("Starting `log` process to listen for video device events...");

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

    println!("Listening for video device events...");

    // Add variables for throttling
    let mut pending_action: Option<tokio::task::JoinHandle<()>> = None;
    let desired_state = std::sync::Arc::new(tokio::sync::Mutex::new(None));

    while let Some(log_line) = reader
        .next_line()
        .await
        .expect("Failed to read log line from `log` process when listening for video device events")
    {
        if !log_line.starts_with("Filtering the log data") {
            if verbose {
                println!("{}", log_line);
            }

            // Update desired state based on the event
            if log_line.contains("AVCaptureSession_Tundra startRunning") {
                println!("Detected that a video device has been turned on.");

                let mut state = desired_state.lock().await;
                *state = Some(true);
            } else if log_line.contains("AVCaptureSession_Tundra stopRunning") {
                println!("Detected that a video device has been turned off.");

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
                        println!("Attempting to turn on Litra device...");
                        let _ = turn_on_first_supported_device_and_log(
                            &mut context_lock,
                            serial_number_clone.as_deref(),
                            require_device,
                        );
                    } else {
                        println!("Attempting to turn off Litra device...");
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

    Err(CliError::IoError(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!(
            "`log` process exited unexpectedly when listening for video device events - {}",
            status
        ),
    )))
}

#[cfg(target_os = "linux")]
fn handle_autotoggle_command(
    serial_number: Option<&str>,
    _verbose: bool,
    require_device: bool,
    video_device: Option<&str>,
) -> CliResult {
    let mut context = Litra::new()?;

    if let Some(device_handle) =
        get_first_supported_device(&mut context, serial_number, require_device)?
    {
        println!(
            "Found {} device (serial number: {})",
            device_handle.device_type(),
            get_serial_number_with_fallback(&device_handle)
        );
    } else {
        print_device_not_found_log(serial_number);
    }

    let mut inotify = Inotify::init()?;
    for path in get_video_device_paths()? {
        if video_device.map_or(true, |device| path.to_str() == Some(device)) {
            match inotify
                .watches()
                .add(&path, WatchMask::OPEN | WatchMask::CLOSE)
            {
                Ok(_) => println!("Watching device {}", path.display()),
                Err(_) => eprintln!("Failed to watch device {}", path.display()),
            }
        }
    }

    let mut num_devices_open: usize = 0;
    loop {
        // Read events that were added with `Watches::add` above.
        let mut buffer = [0; 1024];
        let events = inotify.read_events_blocking(&mut buffer)?;
        for event in events {
            match event.mask {
                EventMask::OPEN => {
                    match event.name.and_then(std::ffi::OsStr::to_str) {
                        Some(name) => println!("Video device opened: {}", name),
                        None => println!("Video device opened"),
                    }
                    num_devices_open = num_devices_open.saturating_add(1);
                }
                EventMask::CLOSE_WRITE | EventMask::CLOSE_NOWRITE => {
                    match event.name.and_then(std::ffi::OsStr::to_str) {
                        Some(name) => println!("Video device closed: {}", name),
                        None => println!("Video device closed"),
                    }
                    num_devices_open = num_devices_open.saturating_sub(1);
                }
                _ => (),
            }
        }
        if num_devices_open == 0 {
            println!("Detected that a video device has been turned off, attempting to turn off Litra device...");

            turn_off_first_supported_device_and_log(&mut context, serial_number, require_device)?;
        } else {
            println!("Detected that a video device has been turned on, attempting to turn on Litra device...");

            turn_on_first_supported_device_and_log(&mut context, serial_number, require_device)?;
        }
    }
}

#[cfg(target_os = "macos")]
#[tokio::main]
async fn main() -> ExitCode {
    let args = Cli::parse();

    let result = handle_autotoggle_command(
        args.serial_number.as_deref(),
        args.verbose,
        args.require_device,
        args.delay,
    )
    .await;

    if let Err(error) = result {
        eprintln!("{}", error);
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(target_os = "linux")]
fn main() -> ExitCode {
    let args = Cli::parse();

    let result = handle_autotoggle_command(
        args.serial_number.as_deref(),
        args.verbose,
        args.require_device,
        args.video_device.as_deref(),
    );

    if let Err(error) = result {
        eprintln!("{}", error);
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
