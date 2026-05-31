use std::{fs::File, os::fd::AsFd};

use cairo::{Context, Format, ImageSurface, Operator};
use memmap2::MmapMut;
use wayland_client::{
    QueueHandle,
    protocol::{
        wl_buffer::WlBuffer,
        wl_shm::{self, WlShm},
    },
};

use crate::{
    app::AppState,
    geometry::{Handle, HitTarget, Rect, cancel_button_rect, select_button_rect, toolbar_rect},
    style::{
        ACCENT, ACCENT_HOVER, HANDLE_FILL, HANDLE_SIZE, MUTED_TEXT, OVERLAY, SECONDARY_BUTTON,
        SECONDARY_BUTTON_HOVER, SHADOW, TEXT, TOOLBAR, TOOLBAR_BORDER,
    },
};

pub struct FrameBuffer {
    pub buffer: WlBuffer,
    context: Context,
    pub busy: bool,
}

impl FrameBuffer {
    pub fn new(
        shm: &WlShm,
        qh: &QueueHandle<AppState>,
        width: i32,
        height: i32,
    ) -> Result<Self, String> {
        let format = Format::ARgb32;
        let stride = format
            .stride_for_width(width as u32)
            .map_err(|error| error.to_string())?;
        let mut file = tempfile::tempfile().map_err(|error| error.to_string())?;
        file.set_len((stride * height) as u64)
            .map_err(|error| error.to_string())?;

        let mmap = map_file(&mut file)?;
        let surface = ImageSurface::create_for_data(mmap, format, width, height, stride)
            .map_err(|error| error.to_string())?;
        let context = Context::new(&surface).map_err(|error| error.to_string())?;
        let pool = shm.create_pool(file.as_fd(), stride * height, qh, ());
        let buffer = pool.create_buffer(0, width, height, stride, wl_shm::Format::Argb8888, qh, ());
        pool.destroy();

        Ok(Self {
            buffer,
            context,
            busy: false,
        })
    }

    pub fn draw(&self, scene: Scene) -> Result<(), String> {
        draw_scene(&self.context, scene).map_err(|error| error.to_string())
    }
}

fn map_file(file: &mut File) -> Result<MmapMut, String> {
    unsafe { MmapMut::map_mut(&*file).map_err(|error| error.to_string()) }
}

#[derive(Debug, Clone, Copy)]
pub struct Scene {
    pub output_bounds: Rect,
    pub selection: Option<Rect>,
    pub active_output: bool,
    pub hovered: HitTarget,
    pub pressing_cancel: bool,
    pub pressing_select: bool,
}

fn draw_scene(context: &Context, scene: Scene) -> Result<(), cairo::Error> {
    paint_background(context, scene.output_bounds)?;

    let Some(selection) = scene.selection.filter(|_| scene.active_output) else {
        draw_hint(context, scene.output_bounds)?;
        return Ok(());
    };

    let local_selection = selection.relative_to(scene.output_bounds);
    clear_selection(context, local_selection)?;
    draw_selection_border(context, local_selection)?;
    draw_handles(context, local_selection, scene.hovered)?;
    draw_toolbar(context, selection, scene)?;
    Ok(())
}

fn paint_background(context: &Context, output: Rect) -> Result<(), cairo::Error> {
    context.set_operator(Operator::Source);
    set_color(context, OVERLAY);
    context.rectangle(0.0, 0.0, output.width, output.height);
    context.fill()
}

fn clear_selection(context: &Context, selection: Rect) -> Result<(), cairo::Error> {
    context.save()?;
    context.set_operator(Operator::Clear);
    context.rectangle(
        selection.x + 1.0,
        selection.y + 1.0,
        (selection.width - 2.0).max(0.0),
        (selection.height - 2.0).max(0.0),
    );
    context.fill()?;
    context.restore()
}

fn draw_selection_border(context: &Context, selection: Rect) -> Result<(), cairo::Error> {
    context.save()?;
    context.set_operator(Operator::Over);
    context.rectangle(selection.x, selection.y, selection.width, selection.height);
    context.set_dash(&[8.0, 7.0], 0.0);
    context.set_line_width(2.0);
    set_color(context, ACCENT);
    context.stroke()?;
    context.restore()
}

fn draw_handles(
    context: &Context,
    selection: Rect,
    hovered: HitTarget,
) -> Result<(), cairo::Error> {
    for handle in Handle::ALL {
        let center = handle.center(selection);
        let is_hovered = hovered == HitTarget::Handle(handle);
        let size = if is_hovered {
            HANDLE_SIZE + 2.0
        } else {
            HANDLE_SIZE
        };
        let rect = Rect::new(center.x - size / 2.0, center.y - size / 2.0, size, size);

        context.save()?;
        rounded_rect(
            context,
            Rect::new(rect.x + 1.0, rect.y + 2.0, rect.width, rect.height),
            3.5,
        );
        set_color(context, SHADOW);
        context.fill()?;

        rounded_rect(context, rect, 3.5);
        set_color(context, HANDLE_FILL);
        context.fill_preserve()?;
        context.set_line_width(1.5);
        set_color(context, if is_hovered { ACCENT_HOVER } else { ACCENT });
        context.stroke()?;
        context.restore()?;
    }
    Ok(())
}

fn draw_toolbar(context: &Context, selection: Rect, scene: Scene) -> Result<(), cairo::Error> {
    let toolbar = toolbar_rect(selection, scene.output_bounds).relative_to(scene.output_bounds);
    let cancel = cancel_button_rect(toolbar);
    let button = select_button_rect(toolbar);
    let cancel_hovered = scene.hovered == HitTarget::CancelButton;
    let button_hovered = scene.hovered == HitTarget::SelectButton;
    let button_fill = if button_hovered { ACCENT_HOVER } else { ACCENT };

    context.save()?;
    rounded_rect(
        context,
        Rect::new(toolbar.x, toolbar.y + 5.0, toolbar.width, toolbar.height),
        13.0,
    );
    set_color(context, SHADOW);
    context.fill()?;

    rounded_rect(context, toolbar, 13.0);
    set_color(context, TOOLBAR);
    context.fill_preserve()?;
    context.set_line_width(1.0);
    set_color(context, TOOLBAR_BORDER);
    context.stroke()?;

    let cancel_pressed_offset = if scene.pressing_cancel && cancel_hovered {
        1.0
    } else {
        0.0
    };
    let visible_cancel = Rect::new(
        cancel.x,
        cancel.y + cancel_pressed_offset,
        cancel.width,
        cancel.height,
    );
    rounded_rect(context, visible_cancel, 10.0);
    set_color(
        context,
        if cancel_hovered {
            SECONDARY_BUTTON_HOVER
        } else {
            SECONDARY_BUTTON
        },
    );
    context.fill()?;
    draw_x(
        context,
        visible_cancel.center().x,
        visible_cancel.center().y,
    )?;

    let pressed_offset = if scene.pressing_select && button_hovered {
        1.0
    } else {
        0.0
    };
    let visible_button = Rect::new(
        button.x,
        button.y + pressed_offset,
        button.width,
        button.height,
    );
    rounded_rect(context, visible_button, 10.0);
    set_color(context, button_fill);
    context.fill()?;

    draw_button_label(context, visible_button, "Select", 11.0)?;

    context.restore()
}

fn draw_hint(context: &Context, output: Rect) -> Result<(), cairo::Error> {
    let chip = Rect::new(
        output.width / 2.0 - 96.0,
        output.height / 2.0 - 22.0,
        192.0,
        44.0,
    );
    context.save()?;
    rounded_rect(context, chip, 12.0);
    set_color(context, TOOLBAR);
    context.fill_preserve()?;
    set_color(context, TOOLBAR_BORDER);
    context.set_line_width(1.0);
    context.stroke()?;
    draw_text_centered(
        context,
        "Drag to select an area",
        chip,
        11.0,
        false,
        ColorRole::Muted,
    )?;
    context.restore()
}

fn draw_button_label(
    context: &Context,
    button: Rect,
    text: &str,
    size: f64,
) -> Result<(), cairo::Error> {
    let layout = text_layout(context, text, size, true);
    let (text_width, text_height) = layout.pixel_size();
    let icon_width = 10.0;
    let gap = 6.0;
    let content_width = icon_width + gap + text_width as f64;
    let content_x = button.x + (button.width - content_width) / 2.0;
    let center_y = button.center().y;
    draw_check(context, content_x + icon_width / 2.0, center_y)?;
    draw_layout(
        context,
        &layout,
        content_x + icon_width + gap,
        center_y - text_height as f64 / 2.0,
        ColorRole::Button,
    )
}

fn draw_check(context: &Context, x: f64, y: f64) -> Result<(), cairo::Error> {
    context.save()?;
    context.move_to(x - 4.0, y);
    context.line_to(x - 1.0, y + 3.0);
    context.line_to(x + 5.0, y - 4.0);
    context.set_line_width(2.0);
    context.set_line_cap(cairo::LineCap::Round);
    context.set_line_join(cairo::LineJoin::Round);
    context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    context.stroke()?;
    context.restore()
}

fn draw_x(context: &Context, x: f64, y: f64) -> Result<(), cairo::Error> {
    context.save()?;
    context.move_to(x - 4.0, y - 4.0);
    context.line_to(x + 4.0, y + 4.0);
    context.move_to(x + 4.0, y - 4.0);
    context.line_to(x - 4.0, y + 4.0);
    context.set_line_width(1.75);
    context.set_line_cap(cairo::LineCap::Round);
    set_color(context, MUTED_TEXT);
    context.stroke()?;
    context.restore()
}

enum ColorRole {
    Muted,
    Button,
}

fn draw_text_centered(
    context: &Context,
    text: &str,
    bounds: Rect,
    size: f64,
    bold: bool,
    role: ColorRole,
) -> Result<(), cairo::Error> {
    let layout = text_layout(context, text, size, bold);
    let (text_width, text_height) = layout.pixel_size();
    draw_layout(
        context,
        &layout,
        bounds.x + (bounds.width - text_width as f64) / 2.0,
        bounds.y + (bounds.height - text_height as f64) / 2.0,
        role,
    )
}

fn text_layout(context: &Context, text: &str, size: f64, bold: bool) -> pango::Layout {
    let layout = pangocairo::functions::create_layout(context);
    let mut font = pango::FontDescription::new();
    font.set_family("Sans");
    font.set_absolute_size(size * pango::SCALE as f64);
    font.set_weight(if bold {
        pango::Weight::Semibold
    } else {
        pango::Weight::Normal
    });
    layout.set_font_description(Some(&font));
    layout.set_text(text);
    layout
}

fn draw_layout(
    context: &Context,
    layout: &pango::Layout,
    x: f64,
    y: f64,
    role: ColorRole,
) -> Result<(), cairo::Error> {
    context.save()?;
    set_color(
        context,
        match role {
            ColorRole::Button => TEXT,
            ColorRole::Muted => MUTED_TEXT,
        },
    );
    context.move_to(x, y);
    pangocairo::functions::show_layout(context, layout);
    context.restore()
}

fn rounded_rect(context: &Context, rect: Rect, radius: f64) {
    let radius = radius.min(rect.width / 2.0).min(rect.height / 2.0);
    let right = rect.right();
    let bottom = rect.bottom();
    context.new_sub_path();
    context.arc(
        right - radius,
        rect.y + radius,
        radius,
        -std::f64::consts::FRAC_PI_2,
        0.0,
    );
    context.arc(
        right - radius,
        bottom - radius,
        radius,
        0.0,
        std::f64::consts::FRAC_PI_2,
    );
    context.arc(
        rect.x + radius,
        bottom - radius,
        radius,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    context.arc(
        rect.x + radius,
        rect.y + radius,
        radius,
        std::f64::consts::PI,
        std::f64::consts::PI * 1.5,
    );
    context.close_path();
}

fn set_color(context: &Context, color: crate::style::Color) {
    context.set_source_rgba(color.red, color.green, color.blue, color.alpha);
}
