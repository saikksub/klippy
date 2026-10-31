use crux_core::{
    App, Command,
    macros::effect,
    render::{RenderOperation, render},
};
use facet::Facet;
use serde::{Deserialize, Serialize};

#[derive(Facet, Serialize, Deserialize, Clone, Debug)]
#[repr(C)]
pub enum Event {
    Increment,
    Decrement,
    Reset,
}

#[effect(facet_typegen)]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
}

#[derive(Default)]
pub struct Model {
    count: isize,
}

#[derive(Facet, Serialize, Deserialize, Clone, Default)]
pub struct ViewModel {
    pub count: String,
}

#[derive(Default)]
pub struct Klippy;

impl App for Klippy {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Effect = Effect;

    fn update(&self, event: Event, model: &mut Model) -> Command<Effect, Event> {
        match event {
            Event::Increment => model.count += 1,
            Event::Decrement => model.count -= 1,
            Event::Reset => model.count = 0,
        }

        render()
    }

    fn view(&self, model: &Model) -> ViewModel {
        ViewModel {
            count: format!("Count is: {}", model.count),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn shows_initial_count() {
        let model = Model::default();
        assert_eq!(Klippy.view(&model).count, "Count is: 0");
    }

    #[test]
    fn increments_and_renders() {
        let mut model = Model::default();
        Klippy.update(Event::Increment, &mut model).expect_only_render();
        assert_eq!(Klippy.view(&model).count, "Count is: 1");
    }

    #[test]
    fn resets() {
        let mut model = Model::default();
        let _ = Klippy.update(Event::Decrement, &mut model);
        let _ = Klippy.update(Event::Reset, &mut model);
        assert_eq!(Klippy.view(&model).count, "Count is: 0");
    }
}
