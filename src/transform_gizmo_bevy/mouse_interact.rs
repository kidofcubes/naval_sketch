use bevy::app::{App, Plugin, Update};
use bevy::ecs::{event::EventWriter, system::Res};
use bevy::input::{mouse::MouseButton, ButtonInput};

use crate::transform_gizmo_bevy::{GizmoDragStarted, GizmoDragging};

pub struct MouseGizmoInteractionPlugin;
impl Plugin for MouseGizmoInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, mouse_interact_gizmo);
    }
}

fn mouse_interact_gizmo(
    mouse: Res<ButtonInput<MouseButton>>,
    mut drag_started: EventWriter<GizmoDragStarted>,
    mut dragging: EventWriter<GizmoDragging>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        drag_started.send_default();
    }

    if mouse.pressed(MouseButton::Left) {
        dragging.send_default();
    }
}
