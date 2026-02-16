use bevy::prelude::*;

use super::grave::SpawnBody;
use super::npc::SpawnNpc;

pub fn plugin(app: &mut App) {
    app.add_observer(on_scenario_trigger);
}

#[derive(Event)]
pub(crate) enum ScenarioTrigger {
    SpawnBody {
        spawner_name: String,
        npc_name: String,
    },
    QueueSpawnBody {
        spawner_name: String,
    },
    SpawnNpc {
        spawner_name: String,
        model: String,
    },
    QueueSpawnNpc {
        spawner_name: String,
    },
}

fn on_scenario_trigger(event: On<ScenarioTrigger>, mut commands: Commands) {
    match &*event {
        ScenarioTrigger::SpawnBody {
            spawner_name,
            npc_name,
        } => {
            commands.trigger(SpawnBody::Direct {
                spawner_name: spawner_name.clone(),
                npc_name: npc_name.clone(),
            });
        }
        ScenarioTrigger::QueueSpawnBody { spawner_name } => {
            commands.trigger(SpawnBody::Queue {
                spawner_name: spawner_name.clone(),
            });
        }
        ScenarioTrigger::SpawnNpc {
            spawner_name,
            model,
        } => {
            commands.trigger(SpawnNpc::Direct {
                spawner_name: spawner_name.clone(),
                model: model.clone(),
            });
        }
        ScenarioTrigger::QueueSpawnNpc { spawner_name } => {
            commands.trigger(SpawnNpc::Queue {
                spawner_name: spawner_name.clone(),
            });
        }
    }
}
