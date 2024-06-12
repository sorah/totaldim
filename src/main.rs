#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // supress console window in release builds
#![allow(deprecated)]

const ICON_DATA: &[u8] = include_bytes!("icon.png");

/// "23b2a129-31ac-4446-b6b0-680c51bba9e4"
const GUID: win_etw_provider::GUID = win_etw_provider::guid!(
    0x23b2a129, 0x31ac, 0x4446, 0xb6, 0xb0, 0x68, 0x0c, 0x51, 0xbb, 0xa9, 0xe4
);

fn main() {
    {
        use tracing_subscriber::prelude::*;
        use tracing_subscriber::Layer;
        let log_fmt = tracing_subscriber::EnvFilter::try_from_default_env()
            .or_else(|_| tracing_subscriber::EnvFilter::try_new("debug"))
            .unwrap()
            .and_then(tracing_subscriber::fmt::layer());
        //let log_etw = win_etw_tracing::TracelogSubscriber::new(GUID, "Totaldim")
        //    .unwrap()
        //    .with_filter(tracing::level_filters::LevelFilter::INFO);
        tracing_subscriber::registry()
            .with(log_fmt)
            //    .with(log_etw)
            .init();
    }
    app()
}

#[tracing::instrument]
fn app() {
    let communicator = Communicator::new("127.0.0.1:7001".parse().unwrap(), None);
    let (action_tx, action_rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        communicator.work(action_rx);
    });

    let event_loop = winit::event_loop::EventLoopBuilder::new().build().unwrap();

    let hotkeys_manager = global_hotkey::GlobalHotKeyManager::new().unwrap();
    let hotkey =
        global_hotkey::hotkey::HotKey::new(None, global_hotkey::hotkey::Code::AudioVolumeMute);
    let hotkey2 = global_hotkey::hotkey::HotKey::new(
        Some(global_hotkey::hotkey::Modifiers::CONTROL),
        global_hotkey::hotkey::Code::F10,
    );

    hotkeys_manager.register(hotkey).unwrap();
    hotkeys_manager.register(hotkey2).unwrap();

    let menu_channel = tray_icon::menu::MenuEvent::receiver();
    let tray_channel = tray_icon::TrayIconEvent::receiver();
    let hotkey_channel = global_hotkey::GlobalHotKeyEvent::receiver();

    let mut tray_instance = None;
    let quit_item = tray_icon::menu::MenuItem::new("Exit", true, None);

    event_loop
        .run(move |event, event_loop| {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::wait_duration(
                std::time::Duration::from_millis(32),
            ));

            if let winit::event::Event::NewEvents(winit::event::StartCause::Init) = event {
                tracing::info!("Initializing");
                let icon = load_icon();
                let menu = tray_icon::menu::Menu::with_items(&[&quit_item]).unwrap();
                tray_instance = Some(
                    tray_icon::TrayIconBuilder::new()
                        .with_menu(Box::new(menu))
                        .with_tooltip("Totaldim")
                        .with_icon(icon)
                        .with_title("Totaldim")
                        .build()
                        .unwrap(),
                );
            };

            if let Ok(e) = hotkey_channel.try_recv() {
                tracing::debug!(event = ?e, "Hotkey Event");
                if (e.id == hotkey.id() || e.id == hotkey2.id())
                    && e.state == global_hotkey::HotKeyState::Pressed
                {
                    tracing::info!("Hotkey pressed");
                    action_tx.send(()).unwrap();
                }
            }

            if let Ok(e) = tray_channel.try_recv() {
                tracing::trace!(event = ?e, "TrayIcon Event");
            }

            if let Ok(e) = menu_channel.try_recv() {
                tracing::debug!(event = ?e, "Menu Event");
                if e.id == quit_item.id() {
                    tracing::info!("Exiting");
                    tray_instance.take();
                    event_loop.exit();
                }
            }
        })
        .unwrap();
}

fn load_icon() -> tray_icon::Icon {
    let (rgba, width, height) = {
        let image = image::load_from_memory(ICON_DATA).unwrap().into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(rgba, width, height).unwrap()
}

#[derive(Debug)]
struct Communicator {
    socket: std::net::UdpSocket,
    destination: std::net::SocketAddr,
    state: bool,
}

impl Communicator {
    fn new(destination: std::net::SocketAddr, local_address: Option<std::net::SocketAddr>) -> Self {
        let local_addr = match local_address {
            Some(a) => a,
            None => match &destination {
                std::net::SocketAddr::V4(_) => std::net::SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
                    0,
                ),
                std::net::SocketAddr::V6(_) => std::net::SocketAddr::new(
                    std::net::IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED),
                    0,
                ),
            },
        };
        let socket = std::net::UdpSocket::bind(local_addr).unwrap();
        Self {
            socket,
            destination,
            state: true,
        }
    }

    fn work(mut self, action_rx: std::sync::mpsc::Receiver<()>) {
        while action_rx.recv().is_ok() {
            self.trigger();
        }
    }

    fn trigger(&mut self) {
        self.state = !self.state;
        let val = if self.state { 1.0 } else { 1.0 };
        let msg = rosc::OscPacket::Message(rosc::OscMessage {
            addr: "/1/mainDim".to_string(),
            args: vec![rosc::OscType::Float(val)],
        });
        tracing::info!(msg = ?msg, me = ?self, "sending OSC message");
        let buf = rosc::encoder::encode(&msg).unwrap();

        match self.socket.send_to(&buf, self.destination) {
            Ok(_) => tracing::debug!("sent"),
            Err(e) => tracing::error!(e = ?e, "error"),
        }
    }
}
