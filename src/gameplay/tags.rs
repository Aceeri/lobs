use bevy::{ecs::entity::EntityHashSet, platform::collections::HashMap, prelude::*};

pub fn plugin(app: &mut App) {
    app.init_resource::<TagIndex>();
    app.add_observer(on_add_tags);
    app.add_observer(on_remove_tags);
}

#[derive(Component, Clone, Debug)]
pub(crate) struct Tags(pub Vec<String>);

impl Tags {
    pub fn from_csv(csv: &str) -> Self {
        Self(
            csv.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        )
    }

    pub fn contains(&self, tag: &str) -> bool {
        self.0.iter().any(|t| t == tag)
    }
}

#[derive(Resource, Default)]
pub(crate) struct TagIndex {
    map: HashMap<String, EntityHashSet>,
}

impl TagIndex {
    pub fn get(&self, tag: &str) -> Option<&EntityHashSet> {
        self.map.get(tag)
    }

    fn insert(&mut self, entity: Entity, tags: &Tags) {
        for tag in &tags.0 {
            self.map.entry(tag.clone()).or_default().insert(entity);
        }
    }

    fn remove(&mut self, entity: Entity, tags: &Tags) {
        for tag in &tags.0 {
            if let Some(set) = self.map.get_mut(tag) {
                set.remove(&entity);
                if set.is_empty() {
                    self.map.remove(tag);
                }
            }
        }
    }
}

fn on_add_tags(add: On<Add, Tags>, mut index: ResMut<TagIndex>, query: Query<&Tags>) {
    if let Ok(tags) = query.get(add.entity) {
        index.insert(add.entity, tags);
    }
}

fn on_remove_tags(remove: On<Remove, Tags>, mut index: ResMut<TagIndex>, query: Query<&Tags>) {
    if let Ok(tags) = query.get(remove.entity) {
        index.remove(remove.entity, tags);
    }
}
