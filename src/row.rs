use egui::{
    epaint, vec2, CursorIcon, InnerResponse, LayerId, Order, PointerButton, Rangef, Rect, Response,
    Sense, Shape, Stroke, Ui, Vec2,
};

use crate::{Interaction, RowLayout, TreeViewSettings};

pub struct Row<NodeIdType> {
    pub id: NodeIdType,
    pub depth: f32,
    pub drop_on_allowed: bool,
    pub is_open: bool,
    pub is_dir: bool,
}

impl<NodeIdType> Row<NodeIdType>
where
    NodeIdType: Clone + Copy + std::hash::Hash,
{
    /// Draw the content as a drag overlay if it is beeing dragged.
    pub(crate) fn draw_row_dragged(
        &self,
        ui: &mut Ui,
        settings: &TreeViewSettings,
        interaction: &Interaction,
        row_response: &Response,
        add_label: &mut dyn FnMut(&mut Ui),
        add_icon: &mut Option<&mut dyn FnMut(&mut Ui)>,
    ) -> bool {
        //*self.drag = Some(self.id);
        ui.ctx().set_cursor_icon(CursorIcon::Alias);

        let drag_source_id = ui.make_persistent_id("Drag source");
        let drag_offset = if interaction.response.drag_started_by(PointerButton::Primary) {
            let drag_offset = ui
                .ctx()
                .pointer_latest_pos()
                .map(|pointer_pos| row_response.rect.min - pointer_pos)
                .unwrap_or(Vec2::ZERO);
            crate::store(ui, drag_source_id, drag_offset);
            drag_offset
        } else {
            crate::load(ui, drag_source_id).unwrap_or(Vec2::ZERO)
        };

        // Paint the content to a new layer for the drag overlay.
        let layer_id = LayerId::new(Order::Tooltip, drag_source_id);

        let background_rect = ui
            .child_ui(ui.available_rect_before_wrap(), *ui.layout())
            .with_layer_id(layer_id, |ui| {
                let background_position = ui.painter().add(Shape::Noop);

                let (row, _, _) = self.draw_row(ui, settings, add_label, add_icon);

                ui.painter().set(
                    background_position,
                    epaint::RectShape::new(
                        row.rect,
                        ui.visuals().widgets.active.rounding,
                        ui.visuals().selection.bg_fill.linear_multiply(0.4),
                        Stroke::NONE,
                    ),
                );
                row.rect
            })
            .inner;

        // Move layer to the drag position
        if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
            let delta = -background_rect.min.to_vec2() + pointer_pos.to_vec2() + drag_offset;
            ui.ctx().translate_layer(layer_id, delta);
        }

        true
    }

    pub(crate) fn draw_row(
        &self,
        ui: &mut Ui,
        settings: &TreeViewSettings,
        add_label: &mut dyn FnMut(&mut Ui),
        add_icon: &mut Option<&mut dyn FnMut(&mut Ui)>,
    ) -> (Response, Option<Response>, Rect) {
        let (reserve_closer, draw_closer, reserve_icon, draw_icon) = match settings.row_layout {
            RowLayout::Compact => (self.is_dir, self.is_dir, false, false),
            RowLayout::CompactAlignedLables => (
                self.is_dir,
                self.is_dir,
                !self.is_dir,
                !self.is_dir && add_icon.is_some(),
            ),
            RowLayout::AlignedIcons => (true, self.is_dir, add_icon.is_some(), add_icon.is_some()),
            RowLayout::AlignedIconsAndLabels => (true, self.is_dir, true, add_icon.is_some()),
        };

        let InnerResponse {
            inner: (closer_response, label_rect_min),
            response: row_response,
        } = ui.horizontal(|ui| {
            ui.add_space(self.depth);
            // The closer and the icon should be drawn vertically centered to the label.
            // To do this we first have to draw the label and then the closer and icon
            // to get the correct position.
            let closer_pos = ui.cursor().min;
            if reserve_closer {
                ui.add_space(ui.spacing().icon_width);
            }

            let icon_pos = ui.cursor().min;
            if reserve_icon {
                ui.add_space(ui.spacing().icon_width);
            };

            let label_pos = ui.cursor().min;
            (add_label)(ui);
            ui.add_space(ui.available_width());

            let closer_response = if draw_closer {
                let (_small_rect, _big_rect) = ui.spacing().icon_rectangles(Rect::from_min_size(
                    closer_pos,
                    vec2(ui.spacing().icon_width, ui.min_size().y),
                ));
                let res = ui.allocate_ui_at_rect(_small_rect, |ui| {
                    let icon_id = ui.make_persistent_id(self.id).with("icon");
                    let openness = ui.ctx().animate_bool(icon_id, self.is_open);
                    let icon_res = ui.allocate_rect(ui.max_rect(), Sense::click());
                    egui::collapsing_header::paint_default_icon(ui, openness, &icon_res);
                    icon_res
                });
                Some(res.inner)
            } else {
                None
            };
            if draw_icon {
                add_icon.as_mut().map(|add_icon| {
                    let (_small_rect, rect) = ui.spacing().icon_rectangles(Rect::from_min_size(
                        icon_pos,
                        vec2(ui.spacing().icon_width, ui.min_size().y),
                    ));
                    ui.allocate_ui_at_rect(rect, |ui| add_icon(ui)).response
                });
            }
            let label_rect_min = if draw_closer {
                closer_pos
            } else if draw_icon {
                icon_pos
            } else {
                label_pos
            };
            (closer_response, label_rect_min.x)
        });

        let background_rect = row_response
            .rect
            .expand2(vec2(0.0, ui.spacing().item_spacing.y * 0.5));
        let label_rect = {
            let mut rect = background_rect.clone();
            rect.min.x = label_rect_min;
            rect
        };

        (
            row_response.with_new_rect(background_rect),
            closer_response,
            label_rect,
        )
    }
}

pub enum DropQuarter {
    Top,
    MiddleTop,
    MiddleBottom,
    Bottom,
}

impl DropQuarter {
    pub fn new(range: Rangef, cursor_pos: f32) -> Option<DropQuarter> {
        pub const DROP_LINE_HOVER_HEIGHT: f32 = 5.0;

        let h0 = range.min;
        let h1 = range.min + DROP_LINE_HOVER_HEIGHT;
        let h2 = (range.min + range.max) / 2.0;
        let h3 = range.max - DROP_LINE_HOVER_HEIGHT;
        let h4 = range.max;

        match cursor_pos {
            y if y >= h0 && y < h1 => Some(Self::Top),
            y if y >= h1 && y < h2 => Some(Self::MiddleTop),
            y if y >= h2 && y < h3 => Some(Self::MiddleBottom),
            y if y >= h3 && y < h4 => Some(Self::Bottom),
            _ => None,
        }
    }
}
