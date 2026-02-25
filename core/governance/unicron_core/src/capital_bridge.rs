use titan::{CapitalState, CapitalEvent};

pub struct TitanBridge {
    pub capital: CapitalState,
}

impl TitanBridge {

    pub fn new() -> Self {
        Self {
            capital: CapitalState::new(),
        }
    }

    pub fn apply(&mut self, event: CapitalEvent) {
        self.capital.apply(event);
    }

    pub fn is_in_safe_mode(&self) -> bool {
        self.capital.safe_mode
    }
}
