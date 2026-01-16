use backend::world_object;
use spacetimedb::{Identity, ReducerContext, Table};

#[spacetimedb::table(name = player, public)]
pub struct Player {
    #[primary_key]
    identity: Identity,
    name: String,
}

#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext) {
    ctx.db.player().insert(Player {
        identity: ctx.sender,
        name: "Player".to_string(),
    });
    // ctx.db.world_object().insert(WorldObject {
    //     identity: ctx.sender,
    //     position: Vec3::new(0.0, 0.0, 0.0),
    // });
}

#[spacetimedb::reducer(client_connected)]
pub fn identity_connected(_ctx: &ReducerContext) {}

#[spacetimedb::reducer(client_disconnected)]
pub fn identity_disconnected(_ctx: &ReducerContext) {}
