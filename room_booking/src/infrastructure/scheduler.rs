//! Lancement périodique des contrôles du couloir Système (rappels, expirations, no-shows…).

use std::{sync::Arc, time::Duration};

use tokio::{
    task::JoinHandle,
    time::{self, MissedTickBehavior},
};

use crate::{
    application::use_cases::run_scheduled_checks::{ScheduledChecksReport, run_scheduled_checks},
    infrastructure::state::{Adapters, AppState},
};

/// Lance une tâche de fond qui exécute `run_scheduled_checks` toutes les `period`.
/// Une erreur est journalisée puis on réessaie au tick suivant : la tâche ne s'arrête jamais.
pub fn spawn_scheduled_checks<A: Adapters>(state: Arc<AppState<A>>, period: Duration) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = time::interval(period);
        // Après une pause (machine en veille…), un seul rattrapage plutôt qu'une rafale.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;
            let _writes = state.lock_writes().await;
            match run_scheduled_checks(&state.bookings, &state.clock, &state.notifier).await {
                Ok(report) if report != ScheduledChecksReport::default() => {
                    println!("contrôles périodiques : {report:?}");
                }
                Ok(_) => {}
                Err(error) => eprintln!("contrôles périodiques en échec : {error}"),
            }
        }
    })
}
