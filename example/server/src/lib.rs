use backend::*;
use spacetimedb::{Identity, ReducerContext, Table};

#[spacetimedb::table(name = player, public)]
pub struct Player {
    #[primary_key]
    identity: Identity,
}

#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext) {
    let _ = ctx.db.world_object().id().find(&1);
}

#[spacetimedb::reducer(client_connected)]
pub fn identity_connected(ctx: &ReducerContext) {
    ctx.db.player().insert(Player {
        identity: ctx.sender,
    });
}

#[spacetimedb::reducer(client_disconnected)]
pub fn identity_disconnected(ctx: &ReducerContext) {
    ctx.db.player().identity().delete(ctx.sender);
}
