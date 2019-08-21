use crate::{
    commands::Result,
    db::database_connection,
    schema::{messages, roles},
};
use diesel::prelude::*;
use serenity::{model::prelude::*, prelude::*};
use std::collections::HashMap;

pub(crate) struct MessageCache;

impl TypeMapKey for MessageCache {
    type Value = HashMap<String, (Message, ChannelId)>;
}

impl MessageCache {
    pub(crate) fn save(name: impl Into<String>, msg: MessageId, channel: ChannelId) -> Result<()> {
        let conn = database_connection()?;

        diesel::insert_into(messages::table)
            .values((
                messages::name.eq(name.into()),
                messages::message.eq(msg.0.to_string()),
                messages::channel.eq(channel.0.to_string()),
            ))
            .execute(&conn)?;
        Ok(())
    }

    pub(crate) fn get_by_name(
        name: impl Into<String>,
    ) -> Result<Option<(i32, String, String, String)>> {
        let conn = database_connection()?;

        Ok(messages::table
            .filter(messages::name.eq(name.into()))
            .load::<(i32, String, String, String)>(&conn)?
            .into_iter()
            .nth(0))
    }

    pub(crate) fn delete_by_name(name: impl Into<String>) -> Result<()> {
        let conn = database_connection()?;
        diesel::delete(messages::table.filter(messages::name.eq(name.into()))).execute(&conn)?;
        Ok(())
    }
}

pub(crate) struct RoleIdCache;

impl TypeMapKey for RoleIdCache {
    type Value = HashMap<String, RoleId>;
}

impl RoleIdCache {
    pub(crate) fn save(name: impl Into<String>, role_id: RoleId) -> Result<()> {
        let conn = database_connection()?;

        diesel::insert_into(roles::table)
            .values((
                roles::role.eq(role_id.0.to_string()),
                roles::name.eq(name.into()),
            ))
            .execute(&conn)?;
        Ok(())
    }

    pub(crate) fn get_by_name(name: impl Into<String>) -> Result<Option<(i32, String, String)>> {
        let conn = database_connection()?;

        Ok(roles::table
            .filter(roles::name.eq(name.into()))
            .load::<(i32, String, String)>(&conn)?
            .into_iter()
            .nth(0))
    }
}
