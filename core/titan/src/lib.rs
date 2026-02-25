use serde::{Serialize, Deserialize};

pub type Timestamp = u64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PropConstraints {
    pub starting_balance: f64,
    pub trailing_drawdown: f64,
    pub daily_loss_limit: f64,
    pub profit_target: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapitalState {
    pub balance: f64,
    pub equity: f64,
    pub high_watermark: f64,
    pub daily_pnl: f64,
    pub total_pnl: f64,
    pub safe_mode: bool,
    pub constraints: PropConstraints,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CapitalEvent {
    Initialize(PropConstraints),
    TradeClosed { pnl: f64, timestamp: Timestamp },
    NewDay,
    ForceSafeMode,
}

impl CapitalState {

    pub fn new() -> Self {
        Self {
            balance: 0.0,
            equity: 0.0,
            high_watermark: 0.0,
            daily_pnl: 0.0,
            total_pnl: 0.0,
            safe_mode: false,
            constraints: PropConstraints {
                starting_balance: 0.0,
                trailing_drawdown: 0.0,
                daily_loss_limit: 0.0,
                profit_target: 0.0,
            },
        }
    }

    pub fn apply(&mut self, event: CapitalEvent) {

        match event {

            CapitalEvent::Initialize(constraints) => {
                self.balance = constraints.starting_balance;
                self.equity = constraints.starting_balance;
                self.high_watermark = constraints.starting_balance;
                self.constraints = constraints;
            }

            CapitalEvent::TradeClosed { pnl, .. } => {

                self.balance += pnl;
                self.equity = self.balance;
                self.total_pnl += pnl;
                self.daily_pnl += pnl;

                if self.equity > self.high_watermark {
                    self.high_watermark = self.equity;
                }

                self.check_risk();
            }

            CapitalEvent::NewDay => {
                self.daily_pnl = 0.0;
            }

            CapitalEvent::ForceSafeMode => {
                self.safe_mode = true;
            }
        }
    }

    fn check_risk(&mut self) {

        let trailing_floor =
            self.high_watermark - self.constraints.trailing_drawdown;

        if self.equity <= trailing_floor {
            self.safe_mode = true;
        }

        if self.daily_pnl <= -self.constraints.daily_loss_limit {
            self.safe_mode = true;
        }

        if self.total_pnl >= self.constraints.profit_target {
            self.safe_mode = true;
        }
    }
}
