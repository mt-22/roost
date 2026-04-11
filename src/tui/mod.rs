pub mod event;
pub mod main_view;
pub mod search;
pub mod state;
pub mod ui;

use color_eyre;

use crate::scanner::SourceEntry;
use state::OnboardingContext;

pub fn run_onboarding(ctx: OnboardingContext) -> color_eyre::Result<Vec<SourceEntry>> {
    let mut tui_state = state::OnboardingTui::new(ctx)?;

    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, &mut tui_state);
    ratatui::restore();

    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut state::OnboardingTui,
) -> color_eyre::Result<Vec<SourceEntry>> {
    loop {
        terminal.draw(|frame| ui::render(state, frame))?;

        match event::handle_event(state)? {
            event::Action::Continue => {}
            event::Action::Finalize => return Ok(state.selected.clone()),
            event::Action::Quit => return Ok(Vec::new()),
        }
    }
}
