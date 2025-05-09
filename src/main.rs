use clap::Parser;
#[cfg(target_os = "linux")]
use inotify::{EventMask, Inotify, WatchMask};
use litra::{Device, DeviceError, DeviceHandle, Litra};
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
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
    
    #[clap(long, help = "Select devices by their type (LitraGlow, LitraBeam, LitraBeamLX)")]
    device_type: Option<String>,
    
    #[clap(long, help = "Apply command to all connected devices", default_value = "false")]
    all_devices: bool,

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

fn check_device_filters(
    serial_number: Option<&str>,
    device_type: Option<&str>,
) -> impl Fn(&Device) -> bool + '_ {
    move |device| {
        // Check device type if specified
        let type_match = device_type.as_ref().map_or(true, |expected| {
            // Convert both to strings without spaces and compare
            let device_type_str = format!("{}", device.device_type())
                .replace(" ", "")
                .to_lowercase();
            
            let expected_type = expected.replace(" ", "").to_lowercase();
            
            // Check if the expected type is contained in the device type (to be more flexible)
            device_type_str.contains(&expected_type) || 
                expected_type.contains(&device_type_str)
        });
        
        // Then check serial number if specified
        let serial_match = serial_number.as_ref().map_or(true, |expected| {
            device
                .device_info()
                .serial_number()
                .is_some_and(|actual| &actual == expected)
        });
        
        // Both filters must match
        type_match && serial_match
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
    device_type: Option<&str>,
    require_device: bool,
) -> Result<Option<DeviceHandle>, CliError> {
    context.refresh_connected_devices()?;

    match context
        .get_connected_devices()
        .find(check_device_filters(serial_number, device_type))
    {
        Some(device) => device
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

fn get_all_supported_devices(
    context: &mut Litra,
    serial_number: Option<&str>,
    device_type: Option<&str>,
) -> Result<Vec<DeviceHandle>, CliError> {
    context.refresh_connected_devices()?;

    let devices: Result<Vec<DeviceHandle>, DeviceError> = context
        .get_connected_devices()
        .filter(check_device_filters(serial_number, device_type))
        .map(|device| device.open(context))
        .collect();

    devices.map_err(CliError::DeviceError)
}

fn turn_on_supported_devices_and_log(
    context: &mut Litra,
    serial_number: Option<&str>,
    device_type: Option<&str>,
    require_device: bool,
    all_devices: bool,
) -> Result<(), CliError> {
    if all_devices {
        let devices = get_all_supported_devices(context, serial_number, device_type)?;
        if devices.is_empty() {
            if require_device {
                return Err(CliError::NoDevicesFound);
            }
            print_device_not_found_log(serial_number);
            return Ok(());
        }
        
        for device_handle in devices {
            println!(
                "Turning on {} device (serial number: {})",
                device_handle.device_type(),
                get_serial_number_with_fallback(&device_handle)
            );
            
            let _ = device_handle.set_on(true); // Ignore errors for individual devices
        }
        Ok(())
    } else {
        if let Some(device_handle) = get_first_supported_device(context, serial_number, device_type, require_device)?
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
}

fn turn_off_supported_devices_and_log(
    context: &mut Litra,
    serial_number: Option<&str>,
    device_type: Option<&str>,
    require_device: bool,
    all_devices: bool,
) -> Result<(), CliError> {
    if all_devices {
        let devices = get_all_supported_devices(context, serial_number, device_type)?;
        if devices.is_empty() {
            if require_device {
                return Err(CliError::NoDevicesFound);
            }
            print_device_not_found_log(serial_number);
            return Ok(());
        }
        
        for device_handle in devices {
            println!(
                "Turning off {} device (serial number: {})",
                device_handle.device_type(),
                get_serial_number_with_fallback(&device_handle)
            );
            
            let _ = device_handle.set_on(false); // Ignore errors for individual devices
        }
        Ok(())
    } else {
        if let Some(device_handle) = get_first_supported_device(context, serial_number, device_type, require_device)?
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
    match device_handle.serial_number() {
        Ok(Some(serial)) if !serial.is_empty() => serial.to_string(),
        _ => {
            // Generate a deterministic serial based on device type and path
            if let Ok(device_info) = device_handle.hid_device().get_device_info() {
                let product_id = device_info.product_id();
                let path = device_info.path().to_string_lossy();
                let device_type_code = match device_handle.device_type() {
                    litra::DeviceType::LitraGlow => "GLOW",
                    litra::DeviceType::LitraBeam => "BEAM",
                    litra::DeviceType::LitraBeamLX => "BEAMLX",
                };
                
                // Create a deterministic identifier using first 12 chars of a hash
                let mut hasher = DefaultHasher::new();
                format!("{}:{}:{}", device_type_code, product_id, path).hash(&mut hasher);
                let hash = hasher.finish();
                
                format!("{:012X}", hash % 0x1000000000000)
            } else {
                "UNKNOWN".to_string()
            }
        }
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
            get_first_supported_device(&mut context_lock, serial_number, None, require_device)?
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
                        let _ = turn_on_supported_devices_and_log(
                            &mut context_lock,
                            serial_number_clone.as_deref(),
                            None,
                            require_device,
                            false,
                        );
                    } else {
                        println!("Attempting to turn off Litra device...");
                        let _ = turn_off_supported_devices_and_log(
                            &mut context_lock,
                            serial_number_clone.as_deref(),
                            None,
                            require_device,
                            false,
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
async fn handle_autotoggle_command(
    serial_number: Option<&str>,
    _verbose: bool,
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
            get_first_supported_device(&mut context_lock, serial_number, None, require_device)?
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
        Ok(_) => println!("Watching {}", watch_path),
        Err(e) => eprintln!("Failed to watch {}: {}", watch_path, e),
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
                    println!("Video device opened: {}", name);
                    num_devices_open = num_devices_open.saturating_add(1);
                }
                EventMask::CLOSE_WRITE | EventMask::CLOSE_NOWRITE => {
                    println!("Video device closed: {}", name);
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
            println!("Detected that a video device has been turned off.");

            let mut state = desired_state.lock().await;
            *state = Some(false);
        } else {
            println!("Detected that a video device has been turned on.");

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
                    println!("Attempting to turn on Litra device...");
                    let _ = turn_on_supported_devices_and_log(
                        &mut context_lock,
                        serial_number_clone.as_deref(),
                        None,
                        require_device,
                        false,
                    );
                } else {
                    println!("Attempting to turn off Litra device...");
                    let _ = turn_off_supported_devices_and_log(
                        &mut context_lock,
                        serial_number_clone.as_deref(),
                        None,
                        require_device,
                        false,
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
#[tokio::main]
async fn main() -> ExitCode {
    let args = Cli::parse();

    let result = handle_autotoggle_command(
        args.serial_number.as_deref(),
        args.verbose,
        args.require_device,
        args.video_device.as_deref(),
        args.delay,
    );

    if let Err(error) = result.await {
        eprintln!("{}", error);
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
