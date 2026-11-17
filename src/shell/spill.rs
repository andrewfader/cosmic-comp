use smithay::{
    desktop::{WindowSurfaceType, space::SpaceElement},
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::Rectangle,
};

use crate::utils::prelude::*;

use super::{CosmicMapped, Shell, focus::spill_sources, grabs::SeatMoveGrabState};

#[derive(Debug, Clone, PartialEq)]
pub struct Spill {
    pub mapped: CosmicMapped,
    pub output: Output,
    overlap: Rectangle<i32, Global>,
}

impl Shell {
    pub fn refresh_spill(&mut self) -> Vec<Output> {
        let current = if self.session_lock.is_some() {
            Vec::new()
        } else {
            self.outputs()
                .flat_map(|output| {
                    spill_sources(self, output).flat_map(move |(layout, _)| {
                        layout.spilling_onto(output).filter_map(move |mapped| {
                            Some(Spill {
                                mapped: mapped.clone(),
                                output: output.clone(),
                                overlap: layout
                                    .element_geometry(mapped)?
                                    .to_global(layout.output())
                                    .intersection(output.geometry())?,
                            })
                        })
                    })
                })
                .collect::<Vec<_>>()
        };

        let mut changed = Vec::new();
        for old in &self.spilled {
            if !current
                .iter()
                .any(|new| new.mapped == old.mapped && new.output == old.output)
            {
                if !self.shows_elsewhere_on(&old.mapped, &old.output) {
                    old.mapped.output_leave(&old.output);
                }
                changed.push(old.output.clone());
            }
        }
        for new in &current {
            let mut overlap = new.overlap.as_logical();
            overlap.loc -= new.mapped.geometry().loc;
            new.mapped.output_enter(&new.output, overlap);
            if !self.spilled.contains(new) {
                changed.push(new.output.clone());
            }
        }
        let focused_output = self.seats.last_active().focused_or_active_output();
        if self.spill_focus.as_ref() != Some(&focused_output) {
            changed.extend(current.iter().map(|spill| spill.output.clone()));
            self.spill_focus = Some(focused_output);
        }
        self.spilled = current;
        changed.dedup();
        changed
    }

    pub fn spill_outputs_for_surface<'a>(
        &'a self,
        surface: &'a WlSurface,
    ) -> impl Iterator<Item = &'a Output> + 'a {
        self.spilled
            .iter()
            .filter(move |spill| spill.mapped.has_surface(surface, WindowSurfaceType::ALL))
            .map(|spill| &spill.output)
    }

    pub fn spilling_onto<'a>(
        &'a self,
        output: &'a Output,
    ) -> impl Iterator<Item = &'a CosmicMapped> + 'a {
        self.spilled
            .iter()
            .filter(move |spill| &spill.output == output)
            .map(|spill| &spill.mapped)
    }

    fn shows_elsewhere_on(&self, mapped: &CosmicMapped, output: &Output) -> bool {
        let dragged = self.seats.iter().any(|seat| {
            seat.user_data()
                .get::<SeatMoveGrabState>()
                .is_some_and(|state| {
                    state
                        .lock()
                        .unwrap()
                        .as_ref()
                        .is_some_and(|grab| &grab.element() == mapped)
                })
        });
        dragged
            || self.workspaces.sets.get(output).is_some_and(|set| {
                set.sticky_layer.mapped().any(|m| m == mapped)
                    || set
                        .workspaces
                        .iter()
                        .any(|workspace| workspace.mapped().any(|m| m == mapped))
            })
    }
}
