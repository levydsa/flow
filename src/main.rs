use bitvec::prelude::*;
use clap::Parser;
use serde::Serialize;
use thiserror::Error;
use wayland_client::{
    delegate_noop,
    protocol::{wl_output, wl_registry, wl_seat},
    Connection, Dispatch, QueueHandle,
};

pub mod river_status {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("./protocols/river-status-unstable-v1.xml");
    }

    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("./protocols/river-status-unstable-v1.xml");
}

use river_status::zriver_output_status_v1;
use river_status::zriver_seat_status_v1;
use river_status::zriver_status_manager_v1;

delegate_noop!(State: ignore zriver_status_manager_v1::ZriverStatusManagerV1);
delegate_noop!(State: ignore wl_output::WlOutput);
delegate_noop!(State: ignore wl_seat::WlSeat);

#[derive(Debug, Default, Clone)]
struct State {
    status_manager: Option<zriver_status_manager_v1::ZriverStatusManagerV1>,
    seat: Option<wl_seat::WlSeat>,
    output: Option<wl_output::WlOutput>,

    changed: bool,

    title: Option<String>,
    mode: Option<String>,
    layout: Option<String>,
    focused: Option<BitVec<u32>>,
    urgent: Option<BitVec<u32>>,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Keep watching for changes
    #[arg(short, long)]
    watch: bool,

    /// Number of tags you want to track
    #[arg(short, long, default_value_t = 9, value_parser = clap::value_parser!(u8).range(1..=32))]
    tags: u8,
}

#[derive(Serialize, Debug, Default, Clone, PartialEq)]
struct Metadata {
    title: String,
    mode: String,
    layout: Option<String>,
    urgent: Vec<bool>,
    focused: Vec<bool>,
}

#[derive(Error, Debug)]
enum Error {
    #[error("missing title in state")]
    MissingTitle,

    #[error("missing mode in state")]
    MissingMode,

    #[error("missing urgent tags list")]
    MissingUrgent,

    #[error("missing focused tags list")]
    MissingFocused,
}

impl TryInto<Metadata> for State {
    type Error = crate::Error;

    fn try_into(self) -> Result<Metadata, Self::Error> {
        Ok(Metadata {
            title: self.title.ok_or_else(|| Error::MissingTitle)?,
            mode: self.mode.ok_or_else(|| Error::MissingMode)?,
            urgent: self
                .urgent
                .ok_or_else(|| Error::MissingUrgent)?
                .into_iter()
                .collect(),
            focused: self
                .focused
                .ok_or_else(|| Error::MissingFocused)?
                .into_iter()
                .collect(),
            layout: self.layout,
        })
    }
}

impl Dispatch<zriver_output_status_v1::ZriverOutputStatusV1, ()> for State {
    fn event(
        state: &mut Self,
        _output_status: &zriver_output_status_v1::ZriverOutputStatusV1,
        event: zriver_output_status_v1::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<State>,
    ) {
        use zriver_output_status_v1::Event as E;
        match event {
            E::FocusedTags { tags } => state.focused = Some(tags.view_bits().to_bitvec()),
            E::UrgentTags { tags } => state.urgent = Some(tags.view_bits().to_bitvec()),

            E::LayoutName { ref name } => state.layout = Some(name.to_owned()),
            E::LayoutNameClear => state.layout = None,
            _ => {}
        }

        state.changed = true;
    }
}

impl Dispatch<zriver_seat_status_v1::ZriverSeatStatusV1, ()> for State {
    fn event(
        state: &mut Self,
        _seat_status: &zriver_seat_status_v1::ZriverSeatStatusV1,
        event: zriver_seat_status_v1::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<State>,
    ) {
        use zriver_seat_status_v1::Event as E;
        match event {
            E::FocusedView { ref title } => state.title = Some(title.to_owned()),
            E::Mode { ref name } => state.mode = Some(name.to_owned()),
            _ => {}
        }

        state.changed = true;
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<State>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version: _version,
        } = event
        {
            match interface.as_str() {
                "wl_output" => {
                    let output = registry.bind::<wl_output::WlOutput, _, _>(name, 4, qh, ());

                    if let Some(ref status_manager) = state.status_manager {
                        status_manager.get_river_output_status(&output, qh, ());
                    }

                    state.output = output.into();
                }
                "wl_seat" => {
                    let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, 4, qh, ());

                    if let Some(ref status_manager) = state.status_manager {
                        status_manager.get_river_seat_status(&seat, qh, ());
                    }

                    state.seat = seat.into();
                }
                "zriver_status_manager_v1" => {
                    use zriver_status_manager_v1::ZriverStatusManagerV1;

                    let status_manager =
                        registry.bind::<ZriverStatusManagerV1, _, _>(name, 4, qh, ());

                    if let Some(ref seat) = state.seat {
                        status_manager.get_river_seat_status(seat, qh, ());
                    }

                    state.status_manager = status_manager.into();
                }
                _ => {}
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let conn = Connection::connect_to_env().unwrap();
    let display = conn.display();

    let mut event_queue = conn.new_event_queue();

    let qh = event_queue.handle();

    display.get_registry(&qh, ());

    let mut state = State { changed: true, ..State::default() };

    loop {
        while state.title.is_none()
            || state.mode.is_none()
            || state.layout.is_none()
            || state.focused.is_none()
            || state.urgent.is_none()
            || !state.changed
        {
            event_queue.blocking_dispatch(&mut state).unwrap();
        }

        let mut metadata: Metadata = state.clone().try_into().unwrap();
        metadata.urgent.truncate(cli.tags.into());
        metadata.focused.truncate(cli.tags.into());

        if state.changed {
            println!("{}", serde_json::to_string(&metadata).unwrap());
            state.changed = false;
        }

        if !cli.watch {
            break;
        }
    }
}
