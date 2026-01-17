use crate::config::ClientRuntimeConfig;
use crate::module_bindings::{
    DbConnection, Reducer, RemoteModule, RemoteReducers, RemoteTables, WorldObjectTableAccess,
    move_object, rotate_object, scale_object,
};
use bevy::prelude::*;
use bevy_spacetimedb::{
    ReadStdbConnectedMessage, RegisterReducerMessage, StdbConnection, StdbPlugin,
};
use spacetimedb_sdk::ReducerEvent;

pub type SpacetimeDB<'a> = Res<'a, StdbConnection<DbConnection>>;

#[allow(dead_code)]
#[derive(Debug, RegisterReducerMessage)]
pub struct MoveObject {
    pub event: ReducerEvent<Reducer>,
    pub id: u64,
    pub translation: crate::module_bindings::Vec3,
}
#[allow(dead_code)]
#[derive(Debug, RegisterReducerMessage)]
pub struct RotateObject {
    pub event: ReducerEvent<Reducer>,
    pub id: u64,
    pub rotation: crate::module_bindings::Quat,
}
#[allow(dead_code)]
#[derive(Debug, RegisterReducerMessage)]
pub struct ScaleObject {
    pub event: ReducerEvent<Reducer>,
    pub id: u64,
    pub scale: crate::module_bindings::Vec3,
}

pub(super) fn plugin(app: &mut App) {
    // Read env-based settings once at startup (the config resource is inserted in `main.rs`)
    // and configure the SpacetimeDB plugin up-front. Avoid any dynamic plugin insertion.
    let config = app
        .world()
        .get_resource::<ClientRuntimeConfig>()
        .expect("ClientRuntimeConfig must be inserted before spacetimedb::plugin is added")
        .clone();

    println!(
        "SpacetimeDB client config: url={:?} module={:?}",
        config.spacetime_url, config.spacetime_name
    );

    app.add_plugins(
        StdbPlugin::default()
            .with_uri(config.spacetime_url)
            .with_module_name(config.spacetime_name)
            .add_table(RemoteTables::world_object)
            .add_reducer::<MoveObject>()
            .add_reducer::<RotateObject>()
            .add_reducer::<ScaleObject>()
            .with_run_fn(DbConnection::run_threaded),
    );

    app.add_systems(Update, on_connect);
}

fn on_connect(mut messages: ReadStdbConnectedMessage, stdb: SpacetimeDB) {
    for message in messages.read() {
        println!("SpacetimeDB module connected: {:?}", message.identity);

        // Subscribe to the tables we actually use.
        let queries = vec!["SELECT * FROM world_object"];
        println!("SpacetimeDB subscribing to: {:?}", queries);

        stdb.subscription_builder().subscribe(queries);
    }
}

impl From<crate::module_bindings::Quat> for Quat {
    fn from(quat: crate::module_bindings::Quat) -> Self {
        Quat::from_xyzw(quat.x, quat.y, quat.z, quat.w)
    }
}
impl From<Quat> for crate::module_bindings::Quat {
    fn from(quat: Quat) -> Self {
        crate::module_bindings::Quat {
            x: quat.x,
            y: quat.y,
            z: quat.z,
            w: quat.w,
        }
    }
}

impl From<crate::module_bindings::Vec3> for Vec3 {
    fn from(vec3: crate::module_bindings::Vec3) -> Self {
        Vec3::new(vec3.x, vec3.y, vec3.z)
    }
}
impl From<Vec3> for crate::module_bindings::Vec3 {
    fn from(vec3: Vec3) -> Self {
        crate::module_bindings::Vec3 {
            x: vec3.x,
            y: vec3.y,
            z: vec3.z,
        }
    }
}
