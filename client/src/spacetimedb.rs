use crate::module_bindings::{DbConnection, RemoteTables, WorldObjectTableAccess};
use bevy::prelude::*;
use bevy_spacetimedb::{ReadStdbConnectedMessage, StdbConnection, StdbPlugin};

pub type SpacetimeDB<'a> = Res<'a, StdbConnection<DbConnection>>;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(
        StdbPlugin::default()
            .with_uri("http://127.0.0.1:3000")
            .with_module_name("example")
            .add_table(RemoteTables::world_object)
            .with_run_fn(DbConnection::run_threaded),
    );
    app.add_systems(Update, on_connect);
}

fn on_connect(mut messages: ReadStdbConnectedMessage, stdb: SpacetimeDB) {
    for message in messages.read() {
        println!("SpacetimeDB module connected: {:?}", message.identity);

        stdb.subscription_builder()
            .subscribe(vec!["SELECT * FROM player", "SELECT * FROM world_object"]);
    }
}
