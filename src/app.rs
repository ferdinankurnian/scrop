use std::cell::OnceCell;

use thiserror::Error;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{
        wl_buffer::{self, WlBuffer},
        wl_compositor::WlCompositor,
        wl_keyboard, wl_output, wl_pointer, wl_registry,
        wl_seat::{self, WlSeat},
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
};
use wayland_protocols::{
    wp::cursor_shape::v1::client::{
        wp_cursor_shape_device_v1::{self, WpCursorShapeDeviceV1},
        wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
    },
    xdg::xdg_output::zv1::client::{zxdg_output_manager_v1::ZxdgOutputManagerV1, zxdg_output_v1},
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, Anchor, ZwlrLayerSurfaceV1},
};

use crate::{
    geometry::{Handle, HitTarget, Interaction, Point, Rect, hit_test},
    render::{FrameBuffer, Scene},
};

const LEFT_MOUSE_BUTTON: u32 = 0x110;
const KEY_ESCAPE: u32 = 1;
const KEY_ENTER: u32 = 28;
const KEY_LEFT_SHIFT: u32 = 42;
const KEY_RIGHT_SHIFT: u32 = 54;
const KEY_UP: u32 = 103;
const KEY_LEFT: u32 = 105;
const KEY_RIGHT: u32 = 106;
const KEY_DOWN: u32 = 108;

#[derive(Debug, Error)]
#[error("{message}")]
pub struct AppError {
    message: String,
}

impl AppError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub fn run(verbose: u8) -> Result<Option<Rect>, AppError> {
    if verbose > 0 {
        eprintln!("scrop: verbose mode level {}", verbose);
    }

    let connection = Connection::connect_to_env()
        .map_err(|error| AppError::new(format!("cannot connect to Wayland: {error}")))?;
    let (globals, _) = registry_queue_init::<AppState>(&connection)
        .map_err(|error| AppError::new(format!("cannot read Wayland globals: {error}")))?;
    let mut event_queue = connection.new_event_queue::<AppState>();
    let qh = event_queue.handle();
    let mut state = AppState::new(qh.clone());

    let compositor = globals
        .bind::<WlCompositor, _, _>(&qh, 1..=5, ())
        .map_err(|error| AppError::new(format!("wl_compositor is unavailable: {error}")))?;
    let shm = globals
        .bind::<WlShm, _, _>(&qh, 1..=1, ())
        .map_err(|error| AppError::new(format!("wl_shm is unavailable: {error}")))?;
    globals
        .bind::<WlSeat, _, _>(&qh, 1..=1, ())
        .map_err(|error| AppError::new(format!("wl_seat is unavailable: {error}")))?;
    state.cursor_manager = globals
        .bind::<WpCursorShapeManagerV1, _, _>(&qh, 1..=1, ())
        .ok();
    state.shm = Some(shm);

    connection.display().get_registry(&qh, ());
    event_queue
        .roundtrip(&mut state)
        .map_err(|error| AppError::new(format!("cannot enumerate outputs: {error}")))?;
    if state.outputs.is_empty() {
        return Err(AppError::new("the compositor reported no outputs"));
    }

    let xdg_output_manager = globals
        .bind::<ZxdgOutputManagerV1, _, _>(&qh, 1..=3, ())
        .map_err(|error| {
            AppError::new(format!("zxdg_output_manager_v1 is unavailable: {error}"))
        })?;
    for output in &mut state.outputs {
        let xdg_output = xdg_output_manager.get_xdg_output(&output.output, &qh, ());
        output
            .xdg
            .set(XdgOutputInfo::new(xdg_output))
            .expect("xdg output is initialized once");
    }
    event_queue
        .roundtrip(&mut state)
        .map_err(|error| AppError::new(format!("cannot read output geometry: {error}")))?;

    let layer_shell = globals
        .bind::<ZwlrLayerShellV1, _, _>(&qh, 1..=4, ())
        .map_err(|error| AppError::new(format!("zwlr_layer_shell_v1 is unavailable: {error}")))?;
    for output in &state.outputs {
        let bounds = output.bounds();
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return Err(AppError::new(format!(
                "output {} has invalid logical geometry",
                output.display_name()
            )));
        }

        let surface = compositor.create_surface(&qh, ());
        let layer = layer_shell.get_layer_surface(
            &surface,
            Some(&output.output),
            Layer::Overlay,
            "scrop".to_owned(),
            &qh,
            (),
        );
        layer.set_anchor(Anchor::all());
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive);
        layer.set_size(bounds.width as u32, bounds.height as u32);
        surface.commit();
        state.surfaces.push(SurfaceInfo::new(layer, surface));
    }

    while state.running {
        event_queue
            .blocking_dispatch(&mut state)
            .map_err(|error| AppError::new(format!("Wayland dispatch failed: {error}")))?;
    }

    layer_shell.destroy();
    for surface in &mut state.surfaces {
        surface.destroy();
    }
    Ok(state.result)
}

#[derive(Debug, Clone)]
struct XdgOutputInfo {
    proxy: zxdg_output_v1::ZxdgOutputV1,
    position: Point,
    width: i32,
    height: i32,
    name: String,
    description: String,
}

impl XdgOutputInfo {
    fn new(proxy: zxdg_output_v1::ZxdgOutputV1) -> Self {
        Self {
            proxy,
            position: Point::default(),
            width: 0,
            height: 0,
            name: String::new(),
            description: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct OutputInfo {
    output: wl_output::WlOutput,
    xdg: OnceCell<XdgOutputInfo>,
    name: String,
    description: String,
}

impl OutputInfo {
    fn new(output: wl_output::WlOutput) -> Self {
        Self {
            output,
            xdg: OnceCell::new(),
            name: String::new(),
            description: String::new(),
        }
    }

    fn xdg(&self) -> &XdgOutputInfo {
        self.xdg.get().expect("xdg output must be initialized")
    }

    fn xdg_mut(&mut self) -> &mut XdgOutputInfo {
        self.xdg.get_mut().expect("xdg output must be initialized")
    }

    fn bounds(&self) -> Rect {
        let xdg = self.xdg();
        Rect::new(
            xdg.position.x,
            xdg.position.y,
            xdg.width as f64,
            xdg.height as f64,
        )
    }

    fn display_name(&self) -> &str {
        if !self.name.is_empty() {
            &self.name
        } else if !self.xdg().name.is_empty() {
            &self.xdg().name
        } else {
            "unknown"
        }
    }
}

struct SurfaceInfo {
    layer: ZwlrLayerSurfaceV1,
    surface: WlSurface,
    width: i32,
    height: i32,
    buffers: Vec<FrameBuffer>,
    rendered_revision: u64,
}

impl SurfaceInfo {
    fn new(layer: ZwlrLayerSurfaceV1, surface: WlSurface) -> Self {
        Self {
            layer,
            surface,
            width: 0,
            height: 0,
            buffers: Vec::new(),
            rendered_revision: 0,
        }
    }

    fn configure(
        &mut self,
        shm: &WlShm,
        qh: &QueueHandle<AppState>,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        if self.width == width && self.height == height && !self.buffers.is_empty() {
            return Ok(());
        }
        for buffer in self.buffers.drain(..) {
            buffer.buffer.destroy();
        }
        self.width = width;
        self.height = height;
        self.rendered_revision = 0;
        self.buffers = (0..2)
            .map(|_| FrameBuffer::new(shm, qh, width, height))
            .collect::<Result<_, _>>()?;
        Ok(())
    }

    fn draw(&mut self, scene: Scene, revision: u64) -> Result<bool, String> {
        if self.rendered_revision == revision {
            return Ok(true);
        }
        let Some(frame) = self.buffers.iter_mut().find(|frame| !frame.busy) else {
            return Ok(false);
        };
        frame.draw(scene)?;
        self.surface.attach(Some(&frame.buffer), 0, 0);
        self.surface.damage(0, 0, self.width, self.height);
        self.surface.commit();
        frame.busy = true;
        self.rendered_revision = revision;
        Ok(true)
    }

    fn destroy(&mut self) {
        for buffer in self.buffers.drain(..) {
            buffer.buffer.destroy();
        }
        self.layer.destroy();
        self.surface.destroy();
    }
}

pub struct AppState {
    qh: QueueHandle<Self>,
    outputs: Vec<OutputInfo>,
    surfaces: Vec<SurfaceInfo>,
    shm: Option<WlShm>,
    cursor_manager: Option<WpCursorShapeManagerV1>,
    pointer: Point,
    current_output: usize,
    active_output: Option<usize>,
    selection: Option<Rect>,
    interaction: Interaction,
    hovered: HitTarget,
    cursor_serial: Option<u32>,
    shift_pressed: bool,
    pointer_bound: bool,
    keyboard_bound: bool,
    revision: u64,
    redraw_pending: bool,
    running: bool,
    result: Option<Rect>,
}

impl AppState {
    fn new(qh: QueueHandle<Self>) -> Self {
        Self {
            qh,
            outputs: Vec::new(),
            surfaces: Vec::new(),
            shm: None,
            cursor_manager: None,
            pointer: Point::default(),
            current_output: 0,
            active_output: None,
            selection: None,
            interaction: Interaction::Idle,
            hovered: HitTarget::Outside,
            cursor_serial: None,
            shift_pressed: false,
            pointer_bound: false,
            keyboard_bound: false,
            revision: 1,
            redraw_pending: false,
            running: true,
            result: None,
        }
    }

    fn bounds(&self, output: usize) -> Rect {
        self.outputs[output].bounds()
    }

    fn set_pointer(&mut self, output: usize, x: f64, y: f64) {
        self.current_output = output;
        let bounds = self.bounds(output);
        self.pointer = Point::new(bounds.x + x, bounds.y + y);
    }

    fn begin_press(&mut self) {
        let hit = self.hit_target();
        self.interaction = match (hit, self.selection) {
            (HitTarget::SelectButton, Some(_)) => Interaction::PressingSelect,
            (HitTarget::CancelButton, Some(_)) => Interaction::PressingCancel,
            (HitTarget::Handle(handle), Some(selection)) => Interaction::Resizing {
                handle,
                selection_origin: selection,
                output: self.current_output,
            },
            (HitTarget::Selection, Some(selection)) => Interaction::Moving {
                pointer_origin: self.pointer,
                selection_origin: selection,
                output: self.current_output,
            },
            _ => {
                self.active_output = Some(self.current_output);
                self.selection = Some(Rect::new(self.pointer.x, self.pointer.y, 0.0, 0.0));
                Interaction::Drawing {
                    anchor: self.pointer,
                    output: self.current_output,
                }
            }
        };
        self.refresh_hover();
        self.invalidate();
    }

    fn update_pointer(&mut self) {
        match self.interaction {
            Interaction::Idle => {}
            Interaction::Drawing { anchor, output } => {
                let point = self.bounds(output).clamp_point(self.pointer);
                self.selection = Some(Rect::from_points(anchor, point));
            }
            Interaction::Moving {
                pointer_origin,
                selection_origin,
                output,
            } => {
                self.selection = Some(selection_origin.translated(
                    self.pointer.x - pointer_origin.x,
                    self.pointer.y - pointer_origin.y,
                    self.bounds(output),
                ));
            }
            Interaction::Resizing {
                handle,
                selection_origin,
                output,
            } => {
                self.selection =
                    Some(selection_origin.resized(handle, self.pointer, self.bounds(output)));
            }
            Interaction::PressingSelect => {}
            Interaction::PressingCancel => {}
        }
        self.refresh_hover();
        self.invalidate();
    }

    fn finish_press(&mut self) {
        let should_confirm = matches!(self.interaction, Interaction::PressingSelect)
            && self.hit_target() == HitTarget::SelectButton;
        let should_cancel = matches!(self.interaction, Interaction::PressingCancel)
            && self.hit_target() == HitTarget::CancelButton;
        if matches!(self.interaction, Interaction::Drawing { .. })
            && self
                .selection
                .is_some_and(|selection| !selection.is_valid())
        {
            self.selection = None;
            self.active_output = None;
        }
        self.interaction = Interaction::Idle;
        self.refresh_hover();
        if should_cancel {
            self.cancel();
        } else if should_confirm {
            self.confirm();
        } else {
            self.invalidate();
        }
    }

    fn hit_target(&self) -> HitTarget {
        match (self.active_output, self.selection) {
            (Some(output), Some(selection)) if output == self.current_output => {
                hit_test(self.pointer, selection, self.bounds(output))
            }
            _ => HitTarget::Outside,
        }
    }

    fn refresh_hover(&mut self) {
        self.hovered = self.hit_target();
    }

    fn confirm(&mut self) {
        if let Some(selection) = self.selection.filter(|selection| selection.is_valid()) {
            self.result = Some(selection);
            self.running = false;
        }
    }

    fn cancel(&mut self) {
        self.result = None;
        self.running = false;
    }

    fn nudge(&mut self, dx: f64, dy: f64) {
        let Some(output) = self.active_output else {
            return;
        };
        let Some(selection) = self.selection else {
            return;
        };
        self.selection = Some(selection.translated(dx, dy, self.bounds(output)));
        self.refresh_hover();
        self.invalidate();
    }

    fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1).max(1);
        self.request_redraw();
    }

    fn request_redraw(&mut self) {
        let mut pending = false;
        for index in 0..self.surfaces.len() {
            let scene = Scene {
                output_bounds: self.bounds(index),
                selection: self.selection,
                active_output: self.active_output == Some(index),
                hovered: if self.active_output == Some(index) {
                    self.hovered
                } else {
                    HitTarget::Outside
                },
                pressing_select: matches!(self.interaction, Interaction::PressingSelect),
                pressing_cancel: matches!(self.interaction, Interaction::PressingCancel),
            };
            match self.surfaces[index].draw(scene, self.revision) {
                Ok(rendered) => pending |= !rendered,
                Err(error) => {
                    eprintln!("scrop: render failed: {error}");
                    self.cancel();
                    return;
                }
            }
        }
        self.redraw_pending = pending;
    }

    fn update_cursor(&self, pointer: &wl_pointer::WlPointer) {
        let (Some(manager), Some(serial)) = (&self.cursor_manager, self.cursor_serial) else {
            return;
        };
        let shape = match self.interaction {
            Interaction::Drawing { .. } => wp_cursor_shape_device_v1::Shape::Crosshair,
            Interaction::Moving { .. } => wp_cursor_shape_device_v1::Shape::Grabbing,
            Interaction::Resizing { handle, .. } => cursor_for_handle(handle),
            Interaction::PressingSelect => wp_cursor_shape_device_v1::Shape::Default,
            Interaction::PressingCancel => wp_cursor_shape_device_v1::Shape::Default,
            Interaction::Idle => match self.hovered {
                HitTarget::Outside => wp_cursor_shape_device_v1::Shape::Crosshair,
                HitTarget::Selection => wp_cursor_shape_device_v1::Shape::Grab,
                HitTarget::Handle(handle) => cursor_for_handle(handle),
                HitTarget::SelectButton => wp_cursor_shape_device_v1::Shape::Default,
                HitTarget::CancelButton => wp_cursor_shape_device_v1::Shape::Default,
            },
        };
        let device = manager.get_pointer(pointer, &self.qh, ());
        device.set_shape(serial, shape);
        device.destroy();
    }

    fn configure_surface(&mut self, layer: &ZwlrLayerSurfaceV1, width: u32, height: u32) {
        let Some(index) = self
            .surfaces
            .iter()
            .position(|surface| surface.layer == *layer)
        else {
            return;
        };
        let output_bounds = self.bounds(index);
        let width = if width == 0 {
            output_bounds.width as i32
        } else {
            width as i32
        };
        let height = if height == 0 {
            output_bounds.height as i32
        } else {
            height as i32
        };
        let shm = self.shm.as_ref().expect("shm is initialized");
        if let Err(error) = self.surfaces[index].configure(shm, &self.qh, width, height) {
            eprintln!("scrop: cannot allocate overlay buffer: {error}");
            self.cancel();
            return;
        }
        self.request_redraw();
    }
}

fn cursor_for_handle(handle: Handle) -> wp_cursor_shape_device_v1::Shape {
    match handle {
        Handle::North | Handle::South => wp_cursor_shape_device_v1::Shape::NsResize,
        Handle::East | Handle::West => wp_cursor_shape_device_v1::Shape::EwResize,
        Handle::NorthWest | Handle::SouthEast => wp_cursor_shape_device_v1::Shape::NwseResize,
        Handle::NorthEast | Handle::SouthWest => wp_cursor_shape_device_v1::Shape::NeswResize,
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for AppState {
    fn event(
        state: &mut Self,
        layer: &ZwlrLayerSurfaceV1,
        event: <ZwlrLayerSurfaceV1 as Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            layer.ack_configure(serial);
            state.configure_surface(layer, width, height);
        }
    }
}

impl Dispatch<zxdg_output_v1::ZxdgOutputV1, ()> for AppState {
    fn event(
        state: &mut Self,
        proxy: &zxdg_output_v1::ZxdgOutputV1,
        event: <zxdg_output_v1::ZxdgOutputV1 as Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some(output) = state
            .outputs
            .iter_mut()
            .find(|output| &output.xdg().proxy == proxy)
        else {
            return;
        };
        let output = output.xdg_mut();
        match event {
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                output.position = Point::new(x as f64, y as f64);
            }
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                output.width = width;
                output.height = height;
            }
            zxdg_output_v1::Event::Name { name } => output.name = name,
            zxdg_output_v1::Event::Description { description } => {
                output.description = description;
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: <wl_registry::WlRegistry as Proxy>::Event,
        _data: &GlobalListContents,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for AppState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
            && interface == wl_output::WlOutput::interface().name
        {
            let output = registry.bind::<wl_output::WlOutput, _, _>(name, version.min(4), qh, ());
            state.outputs.push(OutputInfo::new(output));
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for AppState {
    fn event(
        state: &mut Self,
        proxy: &wl_output::WlOutput,
        event: <wl_output::WlOutput as Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some(output) = state
            .outputs
            .iter_mut()
            .find(|output| &output.output == proxy)
        else {
            return;
        };
        match event {
            wl_output::Event::Name { name } => output.name = name,
            wl_output::Event::Description { description } => output.description = description,
            _ => {}
        }
    }
}

impl Dispatch<WlSeat, ()> for AppState {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: <WlSeat as Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            if capabilities.contains(wl_seat::Capability::Pointer) && !state.pointer_bound {
                seat.get_pointer(qh, ());
                state.pointer_bound = true;
            }
            if capabilities.contains(wl_seat::Capability::Keyboard) && !state.keyboard_bound {
                seat.get_keyboard(qh, ());
                state.keyboard_bound = true;
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for AppState {
    fn event(
        state: &mut Self,
        pointer: &wl_pointer::WlPointer,
        event: <wl_pointer::WlPointer as Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_x,
                surface_y,
            } => {
                let Some(output) = state
                    .surfaces
                    .iter()
                    .position(|candidate| candidate.surface == surface)
                else {
                    return;
                };
                state.cursor_serial = Some(serial);
                state.set_pointer(output, surface_x, surface_y);
                state.refresh_hover();
                state.update_cursor(pointer);
                state.invalidate();
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                state.set_pointer(state.current_output, surface_x, surface_y);
                state.update_pointer();
                state.update_cursor(pointer);
            }
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(button_state),
                ..
            } if button == LEFT_MOUSE_BUTTON => {
                match button_state {
                    wl_pointer::ButtonState::Pressed => state.begin_press(),
                    wl_pointer::ButtonState::Released => state.finish_press(),
                    _ => {}
                }
                state.update_cursor(pointer);
            }
            wl_pointer::Event::Leave { .. } => {
                state.hovered = HitTarget::Outside;
                state.invalidate();
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for AppState {
    fn event(
        state: &mut Self,
        _keyboard: &wl_keyboard::WlKeyboard,
        event: <wl_keyboard::WlKeyboard as Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let wl_keyboard::Event::Key {
            key,
            state: WEnum::Value(key_state),
            ..
        } = event
        else {
            return;
        };
        let pressed = key_state == wl_keyboard::KeyState::Pressed;
        if matches!(key, KEY_LEFT_SHIFT | KEY_RIGHT_SHIFT) {
            state.shift_pressed = pressed;
            return;
        }
        if !pressed {
            return;
        }

        let distance = if state.shift_pressed { 10.0 } else { 1.0 };
        match key {
            KEY_ESCAPE => state.cancel(),
            KEY_ENTER => state.confirm(),
            KEY_UP => state.nudge(0.0, -distance),
            KEY_DOWN => state.nudge(0.0, distance),
            KEY_LEFT => state.nudge(-distance, 0.0),
            KEY_RIGHT => state.nudge(distance, 0.0),
            _ => {}
        }
    }
}

impl Dispatch<WlBuffer, ()> for AppState {
    fn event(
        state: &mut Self,
        buffer: &WlBuffer,
        event: <WlBuffer as Proxy>::Event,
        _data: &(),
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            for surface in &mut state.surfaces {
                if let Some(frame) = surface
                    .buffers
                    .iter_mut()
                    .find(|frame| frame.buffer == *buffer)
                {
                    frame.busy = false;
                }
            }
            if state.redraw_pending {
                state.request_redraw();
            }
        }
    }
}

delegate_noop!(AppState: ignore WlCompositor);
delegate_noop!(AppState: ignore WlSurface);
delegate_noop!(AppState: ignore WlShm);
delegate_noop!(AppState: ignore WlShmPool);
delegate_noop!(AppState: ignore ZwlrLayerShellV1);
delegate_noop!(AppState: ignore ZxdgOutputManagerV1);
delegate_noop!(AppState: ignore WpCursorShapeManagerV1);
delegate_noop!(AppState: ignore WpCursorShapeDeviceV1);
