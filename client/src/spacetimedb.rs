use crate::config::ClientRuntimeConfig;
use crate::module_bindings::{DbConnection, RemoteTables, WorldObjectTableAccess};
use bevy::prelude::*;
use bevy_spacetimedb::{ReadStdbConnectedMessage, StdbConnection, StdbPlugin};

pub type SpacetimeDB<'a> = Res<'a, StdbConnection<DbConnection>>;

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
